use crate::error::{OrchError, OrchResult};
use crate::state::STATE;
use crate::LaunchPlan;
use aws_sdk_ec2::types::Instance;
use aws_sdk_ec2::types::InstanceStateName;
use aws_sdk_ec2::types::InstanceType;
use base64::{engine::general_purpose, Engine as _};
use std::{thread::sleep, time::Duration};

#[derive(Debug)]
pub enum EndpointType {
    Server,
    Client,
}

impl EndpointType {
    pub fn as_str(&self) -> &str {
        match self {
            EndpointType::Server => "Server",
            EndpointType::Client => "Client",
        }
    }
}

pub struct InstanceDetail {
    pub endpoint_type: EndpointType,
    pub instance: aws_sdk_ec2::types::Instance,
    pub ip: String,
}

impl InstanceDetail {
    pub fn new(
        endpoint_type: EndpointType,
        instance: aws_sdk_ec2::types::Instance,
        ip: String,
    ) -> Self {
        InstanceDetail {
            endpoint_type,
            instance,
            ip,
        }
    }

    pub fn instance_id(&self) -> OrchResult<&str> {
        self.instance.instance_id().ok_or(OrchError::Ec2 {
            dbg: "No client id".to_string(),
        })
    }
}

pub async fn launch_instance(
    ec2_client: &aws_sdk_ec2::Client,
    launch_plan: &LaunchPlan,
    name: &str,
) -> OrchResult<aws_sdk_ec2::types::Instance> {
    let instance_type = InstanceType::from(STATE.instance_type);
    let run_result = ec2_client
        .run_instances()
        .key_name(STATE.ssh_key_name)
        .iam_instance_profile(
            aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                .arn(&launch_plan.instance_profile_arn)
                .build(),
        )
        .instance_type(instance_type)
        .image_id(&launch_plan.ami_id)
        .instance_initiated_shutdown_behavior(aws_sdk_ec2::types::ShutdownBehavior::Terminate)
        .user_data(general_purpose::STANDARD.encode(format!(
            "sudo shutdown -P +{}",
            STATE.shutdown_time_sec.as_secs()
        )))
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
                .subnet_id(&launch_plan.subnet_id)
                .groups(&launch_plan.security_group_id)
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

pub async fn delete_instance(ec2_client: &aws_sdk_ec2::Client, ids: Vec<String>) -> OrchResult<()> {
    ec2_client
        .terminate_instances()
        .set_instance_ids(Some(ids))
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;
    Ok(())
}

pub async fn poll_state(
    enumerate: usize,
    endpoint_type: &EndpointType,
    ec2_client: &aws_sdk_ec2::Client,
    instance: &Instance,
    desired_state: InstanceStateName,
) -> OrchResult<String> {
    // Wait for running state
    let mut actual_state = InstanceStateName::Pending;
    let mut ip = None;
    while actual_state != desired_state {
        sleep(Duration::from_secs(5));
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
        actual_state = res.get(0).unwrap().instances().unwrap()[0]
            .state()
            .unwrap()
            .name()
            .unwrap()
            .clone();

        println!(
            "{:?} {} state: {:?}",
            endpoint_type, enumerate, actual_state
        );
    }

    ip.ok_or(crate::error::OrchError::Ec2 {
        dbg: "".to_string(),
    })
}
