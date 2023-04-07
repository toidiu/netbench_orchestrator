/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
#![allow(unused_imports)]
use std::{fmt::format, thread::sleep, time::Duration, collections::HashMap};

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ec2 as ec2;
use aws_sdk_ec2instanceconnect as ec2ic;
use aws_sdk_iam as iam;
use aws_sdk_sqs as sqs;
use aws_sdk_ssm as ssm;
use aws_types::region::Region;
use base64::{engine::general_purpose, Engine as _};
use ec2::types::Filter;

const ORCH_REGION: &str = "us-west-1";
const VPC_REGIONS: [&str; 2] = ["us-east-1", "us-west-2"];

#[tokio::main]
async fn main() -> Result<(), String> {
    /*
     * Overview
     */
    tracing_subscriber::fmt::init();

    //let region_provider = RegionProviderChain::default_provider().or_else("us-west-2");
    let orch_provider = Region::new(VPC_REGIONS[0]);
    let shared_config = aws_config::from_env().region(orch_provider).load().await;

    let ec2_client = ec2::Client::new(&shared_config);
    //let _sqs_client = sqs::Client::new(&shared_config);
    //let _iam_client = iam::Client::new(&shared_config);
    let ec2ic_client = ec2ic::Client::new(&shared_config);
    let ssm_client = ssm::Client::new(&shared_config);

    // Find the Launch Template for the Netbench Runners
    let launch_template = get_launch_template(&ec2_client, "NetbenchRunnerTemplate").await?;

    // Find or define the Subnet to Launch the Netbench Runners
    let subnet_id = get_subnet_ids(&ec2_client, "public-subnet-runners-1").await?;

    /*
     * Launch instances
     *
     * We will define multiple launch templates in CDK for use here.
     *
     * For now: Launch 2 instances with the subnet and launch template.
     */
    let server_details = InstanceDetails {
        launch_template: launch_template.clone(),
        subnet_id: subnet_id.clone(),
    };
    let server = launch(&ec2_client, server_details).await?;

    let client_details = InstanceDetails {
        launch_template: launch_template.clone(),
        subnet_id: subnet_id.clone(),
    };
    let client = launch(&ec2_client, client_details).await?;
    println!("-----Client----");
    //println!("{:#?}", client);
    println!("-----Server----");
    //println!("{:#?}", server);

    /*
     * TODO: Wait for instances to be up in a more sophisticated fashion
     */
    println!("Waiting for instances to be up:");
    //sleep(Duration::new(90, 0));

    /*
     * Setup instances
     */
    let client_instance_id = client.instance_id().map(String::from).ok_or(String::from("No client id"))?;
    let server_instance_id = server.instance_id().map(String::from).ok_or(String::from("No server id"))?;
    let instance_ids = vec![client_instance_id.clone(), server_instance_id.clone()];

    //let instance_ids: Vec<String> = vec!["i-053182c418b960f50", "i-0fcf5bec9f49422a4"].into_iter().map(String::from).collect();
    println!("{:?}", instance_ids);
    let mut parameters = HashMap::new();
    parameters.insert(String::from("commands"), vec![
        "echo starting > /home/ec2-user/working",
        "cd /home/ec2-user",
        "echo su finished > /home/ec2-user/working",
        "sudo yum upgrade -y",
        "sudo yum install cargo git -y",
        "echo yum finished > /home/ec2-user/working",
        "runuser -u ec2-user -- git clone https://github.com/aws/s2n-tls.git",
        "runuser -u ec2-user -- git clone https://github.com/aws/s2n-quic.git",
        "echo git finished > /home/ec2-user/working",
        format!("runuser -u ec2-user -- cat <<- \"HEREDOC\"  > /home/ec2-user/request_response.json \n{}\nHEREDOC", include_str!("request_response.json")).as_str(),
        "echo heredoc finished > /home/ec2-user/working",
        "cd s2n-quic/netbench"
        "runuser -u ec2-user -- cargo build --release",
        "echo build finished > /home/ec2-user/working",
    ].into_iter().map(String::from).collect());
    let mut sent_command_result = Err(String::from("NeverCalled Send Command?"));
    let mut count: u32 = 4;
    let sent_command = loop {
        sent_command_result = ssm_client.send_command().set_instance_ids(Some(instance_ids.clone()))
            .document_name("AWS-RunShellScript").document_version("$LATEST")
            .set_parameters(Some(parameters.clone()))
            .send().await.map_err(|x| format!("{:#?}", x));
        match sent_command_result {
            Ok(sent_command) => {
                break sent_command;
            }
            Err(error_message) => {
                if count > 0 {
                    println!("Error message: {}", error_message);
                    println!("Trying again, waiting 30 seconds...");
                    sleep(Duration::new(30, 0));
                    count -= 1;
                    continue;
                } else {
                    return Err(error_message);
                }
            }
        };
    };

    println!("{:#?}", sent_command);
    /*
     * Retrieve the Secret Key included in launch template
     */
    //let private_

    /*
     * Instance Connect
     *
     * Send Public Key to instances
     */
    let ssh_public_key = include_str!("id_rsa.pub");
    let _ssh_private_key = include_str!("id_rsa");
    // Need instance_id, instance_os_user
    /*
    let client_instance_id = client.instance_id().map(String::from);
    let client_os_user = "ec2-user";
    let client_az = client
        .placement()
        .and_then(ec2::types::Placement::availability_zone)
        .map(String::from);
    let server_instance_id = server.instance_id().map(String::from);
    let server_os_user = "ec2-user";
    let server_az = server
        .placement()
        .and_then(ec2::types::Placement::availability_zone)
        .map(String::from);

    let client_send_ssh = ec2ic_client
        .send_ssh_public_key()
        .set_instance_id(client_instance_id)
        .instance_os_user(client_os_user)
        .set_availability_zone(client_az)
        .ssh_public_key(ssh_public_key)
        .send()
        .await
        .map_err(|r| format!("{:#?}", r))?;
    println!("Send to the client: {:#?}", client_send_ssh);


    if !client_send_ssh.success() {
        print!("Erroring");
        return Err("Sending client the ssh key failed!".into());
    }

    let server_send_ssh = ec2ic_client
        .send_ssh_public_key()
        .set_instance_id(server_instance_id)
        .instance_os_user(server_os_user)
        .set_availability_zone(server_az)
        .ssh_public_key(ssh_public_key)
        .send()
        .await
        .map_err(|r| format!("{:#?}", r))?
        .success();

    if !server_send_ssh {
        print!("Erroring");
        return Err("Sending client the ssh key failed!".into());
    }
    */

    /*
     * Orchestrate Test (Open SSH)
     *
     *   - sudo yum install cargo git
     *   - git clone ...s2n-quic.git
     *   - cd s2n-quic
     *   - cargo build
     *   - ...
     */

    /*
     * Copy results back
     */
    Ok(())
}

/// Find the Launch Template for the Netbench Runners
///  This will be used so that we launch the runners in the right
///  the right security group.
///  NOTE: if you deploy a new version of the launch template, be
///        sure to update the default version
async fn get_launch_template(
    ec2_client: &ec2::Client,
    name: &str,
) -> Result<ec2::types::LaunchTemplateSpecification, String> {
    let launch_template_name = get_launch_template_name(&ec2_client, name).await?;
    Ok(
        ec2::types::builders::LaunchTemplateSpecificationBuilder::default()
            .launch_template_name(launch_template_name)
            .version("$Latest")
            .build(),
    )
}

async fn get_launch_template_name(ec2_client: &ec2::Client, name: &str) -> Result<String, String> {
    let launch_templates: Vec<String> = ec2_client
        .describe_launch_templates()
        .launch_template_names(name)
        .send()
        .await
        .map_err(|r| format!("Describe Launch Template Error: {:#?}", r))?
        .launch_templates()
        .ok_or("No launch templates?")?
        .iter()
        .map(|lt| lt.launch_template_name().unwrap().into())
        .collect();

    if launch_templates.len() == 1 {
        Ok(launch_templates.get(0).unwrap().clone())
    } else {
        Err("Found more launch templates (or none?)".into())
    }
}

// Find or define the Subnet to Launch the Netbench Runners
//  - Default: Use the one defined by CDK
// Note: We may need to define more in different regions and AZ
//      There is some connection between Security Groups and
//      Subnets such that they have to be "in the same network"
//       I'm unclear here.
async fn get_subnet_ids(ec2_client: &ec2::Client, subnet_name: &str) -> Result<String, String> {
    let describe_subnet_output = ec2_client
        .describe_subnets()
        .filters(
            ec2::types::Filter::builder()
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
    Ok(subnet_id.into())
}

/*
 * Launch instance
 *
 * This function launches a single instance. It is configurable using
 * this struct.
 */
struct InstanceDetails {
    launch_template: ec2::types::LaunchTemplateSpecification,
    subnet_id: String,
}
async fn launch(
    ec2_client: &ec2::Client,
    instance_details: InstanceDetails,
) -> Result<ec2::types::Instance, String> {
    let run_result = ec2_client
        .run_instances()
        .launch_template(instance_details.launch_template)
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .subnet_id(instance_details.subnet_id)
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
