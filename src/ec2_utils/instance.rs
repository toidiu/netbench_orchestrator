use crate::error::{OrchError, OrchResult};
use crate::state::STATE;
use crate::LaunchPlan;
use aws_sdk_ec2::types::Instance;
use aws_sdk_ec2::types::InstanceStateName;
use base64::{engine::general_purpose, Engine as _};
use std::{thread::sleep, time::Duration};

pub async fn launch_instance(
    ec2_client: &aws_sdk_ec2::Client,
    instance_details: &LaunchPlan,
    name: &str,
) -> OrchResult<aws_sdk_ec2::types::Instance> {
    let run_result = ec2_client
        .run_instances()
        .key_name(STATE.ssh_key_name)
        .iam_instance_profile(
            aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                .arn(&instance_details.iam_role)
                .build(),
        )
        .instance_type(aws_sdk_ec2::types::InstanceType::C54xlarge)
        .image_id(&instance_details.ami_id)
        .instance_initiated_shutdown_behavior(aws_sdk_ec2::types::ShutdownBehavior::Terminate)
        .user_data(
            general_purpose::STANDARD.encode(format!("sudo shutdown -P +{}", STATE.shutdown_time)),
        )
        // give the instances human readable names. name is set via tags
        .tag_specifications(
            aws_sdk_ec2::types::TagSpecification::builder()
                .resource_type(aws_sdk_ec2::types::ResourceType::Instance)
                .tags(
                    aws_sdk_ec2::types::Tag::builder()
                        .key("Name")
                        .value(name)
                        .build(),
                )
                .build(),
        )
        .block_device_mappings(
            aws_sdk_ec2::types::BlockDeviceMapping::builder()
                .device_name("/dev/xvda")
                .ebs(
                    aws_sdk_ec2::types::EbsBlockDevice::builder()
                        .delete_on_termination(true)
                        .volume_size(50)
                        .build(),
                )
                .build(),
        )
        .network_interfaces(
            aws_sdk_ec2::types::InstanceNetworkInterfaceSpecification::builder()
                .associate_public_ip_address(true)
                .delete_on_termination(true)
                .device_index(0)
                .subnet_id(&instance_details.subnet_id)
                .groups(&instance_details.security_group_id)
                .build(),
        )
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .send()
        .await
        .map_err(|r| crate::error::OrchError::Ec2 {
            dbg: format!("{:#?}", r),
        })?;
    let instances = run_result.instances().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find instances in run result".to_string(),
    })?;
    Ok(instances
        .get(0)
        .ok_or(OrchError::Ec2 {
            dbg: "Didn't launch an instance?".to_string(),
        })?
        .clone())
}

pub async fn wait_for_state(
    ec2_client: &aws_sdk_ec2::Client,
    instance: &Instance,
    desired_state: InstanceStateName,
) -> OrchResult<String> {
    // Wait for running state
    let mut instance_state = InstanceStateName::Pending;
    let mut ip = None;
    while dbg!(instance_state != desired_state) {
        sleep(Duration::from_secs(30));
        let result = ec2_client
            .describe_instances()
            .instance_ids(instance.instance_id().unwrap())
            .send()
            .await
            .unwrap();
        let res = result.reservations().unwrap();
        ip = res
            .get(0)
            .unwrap()
            .instances()
            .unwrap()
            .get(0)
            .unwrap()
            .public_ip_address()
            .map(String::from);
        instance_state = res.get(0).unwrap().instances().unwrap()[0]
            .state()
            .unwrap()
            .name()
            .unwrap()
            .clone()
    }
    // assert_ne!(ip, None);

    ip.ok_or(crate::error::OrchError::Ec2 {
        dbg: "".to_string(),
    })
}

// Find or define the Subnet to Launch the Netbench Runners
//  - Default: Use the one defined by CDK
// Note: We may need to define more in different regions and AZ
//      There is some connection between Security Groups and
//      Subnets such that they have to be "in the same network"
//       I'm unclear here.
pub async fn get_subnet_vpc_ids(
    ec2_client: &aws_sdk_ec2::Client,
    subnet_name: &str,
) -> Result<(String, String), String> {
    let describe_subnet_output = ec2_client
        .describe_subnets()
        .filters(
            aws_sdk_ec2::types::Filter::builder()
                .name("tag:aws-cdk:subnet-name")
                .values(subnet_name)
                .build(),
        )
        .send()
        .await
        .map_err(|e| format!("Couldn't describe subnets: {:#?}", e))?;
    assert_eq!(
        describe_subnet_output.subnets().expect("No subnets?").len(),
        1
    );
    let subnet_id = describe_subnet_output.subnets().unwrap()[0]
        .subnet_id()
        .ok_or::<String>("Couldn't find subnet".into())?;
    let vpc_id = describe_subnet_output.subnets().unwrap()[0]
        .vpc_id()
        .ok_or::<String>("Couldn't find subnet".into())?;
    Ok((subnet_id.into(), vpc_id.into()))
}
