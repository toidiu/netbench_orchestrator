// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use base64::{engine::general_purpose, Engine as _};

struct InstanceDetailsCluster {
    subnet_id: String,
    security_group_id: String,
    ami_id: String,
    iam_role: String,
    placement: aws_sdk_ec2::types::Placement,
}

// Find placement group in infrastructure and use here
async fn launch_cluster(
    client: &aws_sdk_ec2::Client,
    instance_details: InstanceDetailsCluster,
) -> Result<aws_sdk_ec2::types::Instance, String> {
    let run_result = client
        .run_instances()
        .iam_instance_profile(
            aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                .arn(instance_details.iam_role)
                .build(),
        )
        .instance_type(aws_sdk_ec2::types::InstanceType::C5n18xlarge)
        .image_id(instance_details.ami_id)
        .instance_initiated_shutdown_behavior(aws_sdk_ec2::types::ShutdownBehavior::Terminate)
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
//     ec2_client: &aws_sdk_ec2::Client,
//     name: &str,
// ) -> Result<aws_sdk_ec2::types::LaunchTemplateSpecification, String> {
//     let launch_template_name = get_launch_template_name(ec2_client, name).await?;
//     Ok(
//         aws_sdk_ec2::types::builders::LaunchTemplateSpecificationBuilder::default()
//             .launch_template_name(launch_template_name)
//             .version("$Latest")
//             .build(),
//     )
// }

// async fn get_launch_template_name(ec2_client: &aws_sdk_ec2::Client, name: &str) -> Result<String, String> {
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
