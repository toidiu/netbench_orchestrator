use crate::state::STATE;
use aws_sdk_ec2 as ec2;
use base64::{engine::general_purpose, Engine as _};
use std::{thread::sleep, time::Duration};

/*
 * Launch instance
 *
 * This function launches a single instance. It is configurable using
 * this struct.
 */
pub struct InstanceDetails {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub iam_role: String,
}

pub async fn launch_instance(
    ec2_client: &ec2::Client,
    instance_details: InstanceDetails,
    name: &str,
) -> Result<ec2::types::Instance, String> {
    let run_result = ec2_client
        .run_instances()
        .key_name(STATE.ssh_key_name)
        .iam_instance_profile(
            ec2::types::IamInstanceProfileSpecification::builder()
                .arn(instance_details.iam_role)
                .build(),
        )
        .instance_type(ec2::types::InstanceType::C54xlarge)
        .image_id(instance_details.ami_id)
        .instance_initiated_shutdown_behavior(ec2::types::ShutdownBehavior::Terminate)
        .user_data(
            general_purpose::STANDARD.encode(format!("sudo shutdown -P +{}", STATE.shutdown_time)),
        )
        // give the instances human readable names. name is set via tags
        .tag_specifications(
            ec2::types::TagSpecification::builder()
                .resource_type(ec2::types::ResourceType::Instance)
                .tags(ec2::types::Tag::builder().key("Name").value(name).build())
                .build(),
        )
        .block_device_mappings(
            ec2::types::BlockDeviceMapping::builder()
                .device_name("/dev/xvda")
                .ebs(
                    ec2::types::EbsBlockDevice::builder()
                        .delete_on_termination(true)
                        .volume_size(50)
                        .build(),
                )
                .build(),
        )
        .network_interfaces(
            ec2::types::InstanceNetworkInterfaceSpecification::builder()
                .associate_public_ip_address(true)
                .delete_on_termination(true)
                .device_index(0)
                .subnet_id(instance_details.subnet_id)
                .groups(instance_details.security_group_id)
                .build(),
        )
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .send()
        .await
        .map_err(|r| format!("{:#?}", r))?;
    let instances = run_result
        .instances()
        .ok_or::<String>("Couldn't find instances in run result".into())?;
    Ok(instances
        .get(0)
        .ok_or(String::from("Didn't launch an instance?"))?
        .clone())
}

pub async fn delete_security_group(ec2_client: ec2::Client, security_group_id: &str) {
    println!("Start: deleting security groups");
    let mut deleted_sec_group = ec2_client
        .delete_security_group()
        .group_id(security_group_id)
        .send()
        .await;
    sleep(Duration::from_secs(60));

    while deleted_sec_group.is_err() {
        sleep(Duration::from_secs(30));
        deleted_sec_group = ec2_client
            .delete_security_group()
            .group_id(security_group_id)
            .send()
            .await;
    }
    println!("Deleted Security Group: {:#?}", deleted_sec_group);
    println!("Done: deleting security groups");
}

struct InstanceDetailsCluster {
    subnet_id: String,
    security_group_id: String,
    ami_id: String,
    iam_role: String,
    placement: ec2::types::Placement,
}

// Find placement group in infrastructure and use here
async fn launch_cluster(
    client: &ec2::Client,
    instance_details: InstanceDetailsCluster,
) -> Result<ec2::types::Instance, String> {
    let run_result = client
        .run_instances()
        .iam_instance_profile(
            ec2::types::IamInstanceProfileSpecification::builder()
                .arn(instance_details.iam_role)
                .build(),
        )
        .instance_type(ec2::types::InstanceType::C5n18xlarge)
        .image_id(instance_details.ami_id)
        .instance_initiated_shutdown_behavior(ec2::types::ShutdownBehavior::Terminate)
        .user_data(
            general_purpose::STANDARD.encode(format!("sudo shutdown -P +{}", STATE.shutdown_time)),
        )
        .block_device_mappings(
            ec2::types::BlockDeviceMapping::builder()
                .device_name("/dev/xvda")
                .ebs(
                    ec2::types::EbsBlockDevice::builder()
                        .delete_on_termination(true)
                        .volume_size(50)
                        .build(),
                )
                .build(),
        )
        .network_interfaces(
            ec2::types::InstanceNetworkInterfaceSpecification::builder()
                .associate_public_ip_address(true)
                .delete_on_termination(true)
                .device_index(0)
                .subnet_id(instance_details.subnet_id)
                .groups(instance_details.security_group_id)
                .build(),
        )
        .placement(instance_details.placement)
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .send()
        .await
        .map_err(|r| format!("{:#?}", r))?;
    Ok(run_result
        .instances()
        .ok_or::<String>("Couldn't find instances in run result".into())?
        .get(0)
        .ok_or::<String>("Couldn't find instances in run result".into())?
        .clone())
}

// TODO waiting to see if this is needed for multiple hosts.. else delete
// /// Find the Launch Template for the Netbench Runners
// ///  This will be used so that we launch the runners in the right
// ///  the right security group.
// ///  NOTE: if you deploy a new version of the launch template, be
// ///        sure to update the default version
// async fn get_launch_template(
//     ec2_client: &ec2::Client,
//     name: &str,
// ) -> Result<ec2::types::LaunchTemplateSpecification, String> {
//     let launch_template_name = get_launch_template_name(ec2_client, name).await?;
//     Ok(
//         ec2::types::builders::LaunchTemplateSpecificationBuilder::default()
//             .launch_template_name(launch_template_name)
//             .version("$Latest")
//             .build(),
//     )
// }

// async fn get_launch_template_name(ec2_client: &ec2::Client, name: &str) -> Result<String, String> {
//     let launch_templates: Vec<String> = ec2_client
//         .describe_launch_templates()
//         .launch_template_names(name)
//         .send()
//         .await
//         .map_err(|r| format!("Describe Launch Template Error: {:#?}", r))?
//         .launch_templates()
//         .ok_or("No launch templates?")?
//         .iter()
//         .map(|lt| lt.launch_template_name().unwrap().into())
//         .collect();

//     if launch_templates.len() == 1 {
//         Ok(launch_templates.get(0).unwrap().clone())
//     } else {
//         Err("Found more launch templates (or none?)".into())
//     }
// }
