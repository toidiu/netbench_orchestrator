/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
#![allow(unused_imports)]
use tempdir::TempDir;

use std::{collections::HashMap, fmt::format, thread::sleep, time::Duration};

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    BufReader::new(File::open(filename)?).lines().collect()
}

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ec2 as ec2;
use aws_sdk_ec2instanceconnect as ec2ic;
use aws_sdk_iam as iam;
use aws_sdk_sqs as sqs;
use aws_sdk_ssm as ssm;
use aws_types::region::Region;
use base64::{engine::general_purpose, Engine as _};
use ec2::types::Filter;
use iam::types::StatusType;
use ssm::operation::send_command::SendCommandOutput;

const ORCH_REGION: &str = "us-west-1";
const VPC_REGIONS: [&str; 2] = ["us-east-1", "us-west-2"];

#[tokio::main]
async fn main() -> Result<(), String> {
    /*
     * Overview
     */
    tracing_subscriber::fmt::init();

    let unique_id = format!("test-{}", std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos());

    //let region_provider = RegionProviderChain::default_provider().or_else("us-west-2");
    let orch_provider = Region::new(ORCH_REGION);
    let shared_config = aws_config::from_env().region(orch_provider).load().await;

    let ec2_client = ec2::Client::new(&shared_config);
    //let _sqs_client = sqs::Client::new(&shared_config);
    let iam_client = iam::Client::new(&shared_config);
    //let ec2ic_client = ec2ic::Client::new(&shared_config);

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

    let orch_provider_vpc = Region::new(VPC_REGIONS[0]);
    let shared_config_vpc = aws_config::from_env()
        .region(orch_provider_vpc)
        .load()
        .await;
    let ec2_vpc = ec2::Client::new(&shared_config_vpc);
    let ssm_client = ssm::Client::new(&shared_config_vpc);

    // Find the Launch Template for the Netbench Runners
    // let launch_template = get_launch_template(&ec2_vpc, "NetbenchRunnerTemplate-us-east-1").await?;

    // Find or define the Subnet to Launch the Netbench Runners
    let (subnet_id, vpc_id) =
        get_subnet_vpc_ids(&ec2_vpc, "public-subnet-for-runners-in-us-east-1").await?;

    // Create a security group
    let security_group_id: String = ec2_vpc
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

    /*
     * Launch instances
     *
     * We will define multiple launch templates in CDK for use here.
     *
     * For now: Launch 2 instances with the subnet and launch template.
     */
    let server_details = InstanceDetails {
        ami_id: ami_id.clone(),
        subnet_id: subnet_id.clone(),
        security_group_id: security_group_id.clone(),
        iam_role: iam_role.clone(),
    };
    let server = launch(&ec2_vpc, server_details).await?;

    let client_details = InstanceDetails {
        ami_id: ami_id.clone(),
        subnet_id: subnet_id.clone(),
        security_group_id: security_group_id.clone(),
        iam_role: iam_role.clone(),
    };
    let client = launch(&ec2_vpc, client_details).await?;
    println!("-----Client----");
    //println!("{:#?}", client);
    println!("-----Server----");
    //println!("{:#?}", server);

    /*
     * Wait for running state
     */
    let mut client_code = 0;
    let mut ip_client = None;
    while dbg!(client_code != 16) {
        sleep(Duration::from_secs(30));
        let result = ec2_vpc
            .describe_instances()
            .instance_ids(client.instance_id().unwrap())
            .send()
            .await
            .unwrap();
        let res = result.reservations().unwrap();
        ip_client = res
            .get(0)
            .unwrap()
            .instances()
            .unwrap()
            .get(0)
            .unwrap()
            .public_ip_address()
            .map(String::from);
        client_code = res.get(0).unwrap().instances().unwrap()[0]
            .state()
            .unwrap()
            .code()
            .unwrap()
    }
    assert_ne!(ip_client, None);

    let mut server_code = 0;
    let mut ip_server = None;
    while dbg!(server_code != 16) {
        sleep(Duration::from_secs(30));
        let result = ec2_vpc
            .describe_instances()
            .instance_ids(server.instance_id().unwrap())
            .send()
            .await
            .unwrap();
        let res = result.reservations().unwrap();
        ip_server = res
            .get(0)
            .unwrap()
            .instances()
            .unwrap()
            .get(0)
            .unwrap()
            .public_ip_address()
            .map(String::from);
        server_code = res.get(0).unwrap().instances().unwrap()[0]
            .state()
            .unwrap()
            .code()
            .unwrap()
    }
    assert_ne!(ip_server, None);

    /*
     * Modify Security Group
     */
    let client_ip: String = ip_client.unwrap();
    println!("client ip: {:#?}", client_ip);
    let server_ip: String = ip_server.unwrap();
    println!("server ip: {:#?}", server_ip);

    let x = ec2_vpc
        .authorize_security_group_egress()
        .group_id(security_group_id.clone())
        .ip_permissions(
            ec2::types::IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", client_ip))
                        .build(),
                )
                .ip_ranges(
                    ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", server_ip))
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .expect("error");
    let x = ec2_vpc
        .authorize_security_group_ingress()
        .group_id(security_group_id.clone())
        .ip_permissions(
            ec2::types::IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", client_ip))
                        .build(),
                )
                .ip_ranges(
                    ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", server_ip))
                        .build(),
                )
                .build(),
        )
        .ip_permissions(
            ec2::types::IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(ec2::types::IpRange::builder().cidr_ip("0.0.0.0/0").build())
                .build(),
        )
        .send()
        .await
        .expect("error");

    /*
     * Setup instances
     */
    let client_instance_id = client
        .instance_id()
        .map(String::from)
        .ok_or(String::from("No client id"))?;
    let server_instance_id = server
        .instance_id()
        .map(String::from)
        .ok_or(String::from("No server id"))?;
    //let client_instance_id = String::from("i-07b0a35b017f5370b");
    //let server_instance_id = String::from("i-07eda896bf2f7e38c");
    let instance_ids = vec![client_instance_id.clone(), server_instance_id.clone()];

    //let instance_ids: Vec<String> = vec!["i-053182c418b960f50", "i-0fcf5bec9f49422a4"].into_iter().map(String::from).collect();
    println!("{:?}", instance_ids);

    let send_command_output_client = send_command(&ssm_client, client_instance_id, vec![
        "echo starting > /home/ec2-user/working",
        "cd /home/ec2-user",
        "echo su finished > /home/ec2-user/working",
        "yum upgrade -y",
        "yum install cargo git perl openssl-devel bpftrace perf tree -y",
        "echo yum finished > /home/ec2-user/working",
        "runuser -u ec2-user -- git clone --branch netbench_sync https://github.com/harrisonkaiser/s2n-quic.git",
        "runuser -u ec2-user -- echo git finished > /home/ec2-user/working",
        format!("runuser -u ec2-user -- cat <<- \"HEREDOC\"  > /home/ec2-user/request_response.json \n{}\nHEREDOC", include_str!("request_response.json")).as_str(),
        "runuser -u ec2-user -- echo heredoc finished > /home/ec2-user/working",
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        "runuser -u ec2-user -- echo build finished > /home/ec2-user/working",
        format!("env SERVER_0={}:4433 COORD_SERVER_0={}:8080 ./scripts/netbench-client.sh", server_ip, server_ip).as_str(),
        "chown ec2-user: -R .",
        "runuser -u ec2-user -- echo build finished > /home/ec2-user/working",
        "runuser -u ec2-user -- cd target/netbench",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        "shutdown -h +1",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out");

    let send_command_output_server = send_command(&ssm_client, server_instance_id.clone(), vec![
        "runuser -u ec2-user -- echo starting > /home/ec2-user/working",
        "cd /home/ec2-user",
        "runuser -u ec2-user -- echo su finished > /home/ec2-user/working",
        "yum upgrade -y",
        "yum install cargo git perl openssl-devel bpftrace perf tree -y",
        "runuser -u ec2-user -- echo yum finished > /home/ec2-user/working",
        "runuser -u ec2-user -- git clone --branch netbench_sync https://github.com/harrisonkaiser/s2n-quic.git",
        "runuser -u ec2-user -- echo git finished > /home/ec2-user/working",
        format!("runuser -u ec2-user -- cat <<- \"HEREDOC\"  > /home/ec2-user/request_response.json \n{}\nHEREDOC", include_str!("request_response.json")).as_str(),
        "runuser -u ec2-user -- echo heredoc finished > /home/ec2-user/working",
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        "runuser -u ec2-user -- echo build finished > /home/ec2-user/working",
        format!("env COORD_CLIENT_0={}:8080 ./scripts/netbench-server.sh", client_ip).as_str(),
        "chown ec2-user: -R .",
        "runuser -u ec2-user -- echo build finished > /home/ec2-user/working",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out");
    let ssm_command_result_client = wait_for_ssm_results(
        &ssm_client,
        send_command_output_client
            .command()
            .unwrap()
            .command_id()
            .unwrap()
            .into(),
    )
    .await;
    println!("Client Finished!: Successful: {}", ssm_command_result_client);
    let ssm_command_result_server = wait_for_ssm_results(
        &ssm_client,
        send_command_output_server
            .command()
            .unwrap()
            .command_id()
            .unwrap()
            .into(),
    ).await;
    println!("Server Finished!: Successful: {}", ssm_command_result_server);

    /*
     * Copy results back
     */
    let generate_report = dbg!(send_command(&ssm_client, server_instance_id, vec![
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/before-sync",
        format!("runuser -u ec2-user -- aws s3 sync s3://netbenchrunnerlogs/{} /home/ec2-user/s2n-quic/netbench/target/netbench", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync",
        "cd /home/ec2-user/s2n-quic/netbench/",
        "runuser -u ec2-user -- ./target/release/netbench-cli report-tree ./target/netbench/results ./target/netbench/report",
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-report",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync-back",
        "shutdown -h +1",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out"));
    let report_result = wait_for_ssm_results(
        &ssm_client,
        generate_report
            .command()
            .unwrap()
            .command_id()
            .unwrap()
            .into(),
    ).await;
    println!("Report Finished!: Successful: {}", report_result);

    println!("URL: http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html", unique_id);


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
async fn get_subnet_vpc_ids(
    ec2_client: &ec2::Client,
    subnet_name: &str,
) -> Result<(String, String), String> {
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
    let vpc_id = describe_subnet_output.subnets().unwrap()[0]
        .vpc_id()
        .ok_or::<String>("Couldn't find subnet".into())?;
    Ok((subnet_id.into(), vpc_id.into()))
}

/*
 * Launch instance
 *
 * This function launches a single instance. It is configurable using
 * this struct.
 */
struct InstanceDetails {
    subnet_id: String,
    security_group_id: String,
    ami_id: String,
    iam_role: String,
}
async fn launch(
    ec2_client: &ec2::Client,
    instance_details: InstanceDetails,
) -> Result<ec2::types::Instance, String> {
    let run_result = ec2_client
        .run_instances()
        .iam_instance_profile(
            ec2::types::IamInstanceProfileSpecification::builder()
                .arn(instance_details.iam_role)
                .build(),
        )
        .instance_type(ec2::types::InstanceType::M416xlarge)
        .image_id(instance_details.ami_id)
        .instance_initiated_shutdown_behavior(ec2::types::ShutdownBehavior::Terminate)
        .user_data(general_purpose::STANDARD.encode("sudo shutdown -P +240"))
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

async fn send_command(
    ssm_client: &ssm::Client,
    instance_id: String,
    commands: Vec<String>,
) -> Option<SendCommandOutput> {
    let mut remaining_try_count: u32 = 30;
    loop {
        match ssm_client
            .send_command()
            .instance_ids(instance_id.clone())
            .document_name("AWS-RunShellScript")
            .document_version("$LATEST")
            .parameters("commands", commands.clone())
            .cloud_watch_output_config(
                ssm::types::CloudWatchOutputConfig::builder()
                    .cloud_watch_log_group_name("hello")
                    .cloud_watch_output_enabled(true)
                    .build(),
            )
            .send()
            .await
            .map_err(|x| format!("{:#?}", x))
        {
            Ok(sent_command) => {
                break Some(sent_command);
            }
            Err(error_message) => {
                if remaining_try_count > 0 {
                    println!("Error message: {}", error_message);
                    println!("Trying again, waiting 30 seconds...");
                    sleep(Duration::new(30, 0));
                    remaining_try_count -= 1;
                    continue;
                } else {
                    return None;
                }
            }
        };
    }
}

async fn wait_for_ssm_results(ssm_client: &ssm::Client, command_id: String) -> bool {
    loop {
        let o_status = ssm_client
            .list_command_invocations()
            .command_id(command_id.clone())
            .send()
            .await
            .unwrap()
            .command_invocations()
            .unwrap()
            .iter()
            .filter_map(|command| command.status())
            .next().cloned();
        let status = match o_status {
            Some(s) => s,
            None => return true,
        };
        match status {
            ssm::types::CommandInvocationStatus::Cancelled
            | ssm::types::CommandInvocationStatus::Cancelling
            | ssm::types::CommandInvocationStatus::Failed
            | ssm::types::CommandInvocationStatus::TimedOut => break false,
            ssm::types::CommandInvocationStatus::Delayed
            | ssm::types::CommandInvocationStatus::InProgress
            | ssm::types::CommandInvocationStatus::Pending => {
                dbg!(status);
                sleep(Duration::from_secs(30));
                continue;
            }
            ssm::types::CommandInvocationStatus::Success => break true,
            _ => panic!("Unhandled Status"),
        };
    }
}
