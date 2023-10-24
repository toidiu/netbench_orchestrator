use aws_sdk_ec2 as ec2;
use aws_sdk_iam as iam;
use aws_sdk_ssm as ssm;
use std::{thread::sleep, time::Duration};

use self::instance::wait_for_state;

mod cluster;
mod instance;

#[derive(Clone)]
pub struct InstanceDetails {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub iam_role: String,
}

impl InstanceDetails {
    pub async fn new(
        unique_id: &str,
        ec2_client: &ec2::Client,
        iam_client: &iam::Client,
        ssm_client: &ssm::Client,
    ) -> Self {
        let iam_role: String = iam_client
            .get_instance_profile()
            .instance_profile_name("NetbenchRunnerInstanceProfile")
            .send()
            .await
            .unwrap()
            .instance_profile()
            .unwrap()
            .arn()
            .unwrap()
            .into();

        // Find or define the Subnet to Launch the Netbench Runners
        let (subnet_id, vpc_id) =
            instance::get_subnet_vpc_ids(&ec2_client, "public-subnet-for-runners-in-us-east-1")
                .await
                .unwrap();

        // Create a security group
        let security_group_id: String = ec2_client
            .create_security_group()
            .group_name(format!("generated_group_{}", unique_id))
            .description("This is a security group for a single run of netbench.")
            .vpc_id(vpc_id)
            .send()
            .await
            .expect("No output?")
            .group_id()
            .expect("No group ID?")
            .into();

        // Get latest ami
        let ami_id: String = ssm_client
            .get_parameter()
            .name("/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64")
            .with_decryption(true)
            .send()
            .await
            .unwrap()
            .parameter()
            .unwrap()
            .value()
            .unwrap()
            .into();

        InstanceDetails {
            ami_id,
            subnet_id,
            security_group_id,
            iam_role,
        }
    }
}

pub async fn launch_server_client(
    ec2_client: &ec2::Client,
    instance_details: &InstanceDetails,
    unique_id: &str,
) -> Result<(ec2::types::Instance, ec2::types::Instance), String> {
    let server = format!("server-{}", unique_id);
    let client = format!("client-{}", unique_id);

    let server = instance::launch_instance(ec2_client, instance_details, &server).await?;
    let client = instance::launch_instance(ec2_client, instance_details, &client).await?;

    wait_for_state(ec2_client, &server, ec2::types::InstanceStateName::Running).await;
    wait_for_state(ec2_client, &client, ec2::types::InstanceStateName::Running).await;

    Ok((server, client))
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
