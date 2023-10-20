/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
#![allow(unused_imports)]
use bytes::Bytes;
use std::borrow::BorrowMut;
use std::{collections::HashMap, fmt::format, thread::sleep, time::Duration};
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};
use tempdir::TempDir;

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    BufReader::new(File::open(filename)?).lines().collect()
}

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ec2 as ec2;
use aws_sdk_ec2::types::InstanceStateName;
use aws_sdk_ec2instanceconnect as ec2ic;
use aws_sdk_iam as iam;
use aws_sdk_s3 as s3;
use aws_sdk_sqs as sqs;
use aws_sdk_ssm as ssm;

use aws_types::region::Region;
use base64::{engine::general_purpose, Engine as _};
use ec2::types::Filter;
use iam::types::StatusType;

mod launch;
mod state;
mod utils;
mod execute_on_host;

use launch::*;
use state::*;
use utils::*;

#[tokio::main]
async fn main() -> Result<(), String> {
    /*
     * Overview
     */
    tracing_subscriber::fmt::init();

    let unique_id = format!(
        "{}-{}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
        STATE.version
    );

    let status = format!(
        "http://d2jusruq1ilhjs.cloudfront.net/{}/index.html",
        unique_id
    );
    let template_server_prefix = format!("{}/server-step-", STATE.cf_url_with_id(&unique_id));
    let template_client_prefix = format!("{}/client-step-", STATE.cf_url_with_id(&unique_id));
    let template_finished_prefix = format!("{}/finished-step-", STATE.cf_url_with_id(&unique_id));

    // Upload a status file to s3:
    let index_file = std::fs::read_to_string("index.html")
        .unwrap()
        .replace("template_unique_id", &unique_id)
        .replace("template_server_prefix", &template_server_prefix)
        .replace("template_client_prefix", &template_client_prefix)
        .replace("template_finished_prefix", &template_finished_prefix);

    let orch_provider = Region::new(STATE.region);
    let shared_config = aws_config::from_env().region(orch_provider).load().await;
    let iam_client = iam::Client::new(&shared_config);
    let s3_client = s3::Client::new(&shared_config);

    let _ = s3_client
        .put_object()
        .body(s3::primitives::ByteStream::from(Bytes::from(index_file)))
        .bucket(STATE.log_bucket)
        .key(format!("{unique_id}/index.html"))
        .content_type("text/html")
        .send()
        .await
        .unwrap();

    println!("Status: URL: {status}");

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

    let orch_provider_vpc = Region::new(STATE.vpc_region);
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
    let server = launch_instance(
        &ec2_vpc,
        server_details,
        format!("server-{}", unique_id).as_str(),
    )
    .await?;

    let client_details = InstanceDetails {
        ami_id: ami_id.clone(),
        subnet_id: subnet_id.clone(),
        security_group_id: security_group_id.clone(),
        iam_role: iam_role.clone(),
    };
    let client = launch_instance(
        &ec2_vpc,
        client_details,
        format!("client-{}", unique_id).as_str(),
    )
    .await?;
    println!("-----Client----");
    //println!("{:#?}", client);
    println!("-----Server----");
    //println!("{:#?}", server);

    /*
     * Wait for running state
     */
    let mut client_code = InstanceStateName::Pending;
    let mut ip_client = None;
    while dbg!(client_code != InstanceStateName::Running) {
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
            .name()
            .unwrap()
            .clone()
    }
    assert_ne!(ip_client, None);

    let mut server_code = InstanceStateName::Pending;
    let mut ip_server = None;
    while dbg!(server_code != InstanceStateName::Running) {
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
            .name()
            .unwrap()
            .clone()
    }
    assert_ne!(ip_server, None);

    /*
     * Modify Security Group
     */
    let client_ip: String = ip_client.unwrap();
    println!("client ip: {:#?}", client_ip);
    let server_ip: String = ip_server.unwrap();
    println!("server ip: {:#?}", server_ip);

    let _network_perms = ec2_vpc
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
    let _network_perms = ec2_vpc
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

    let instance_ids = vec![client_instance_id.clone(), server_instance_id.clone()];

    let _ = s3_client
        .put_object()
        .body(s3::primitives::ByteStream::from(Bytes::from(format!(
            "EC2 Server Runner up: {}",
            server_instance_id.clone()
        ))))
        .bucket(STATE.log_bucket)
        .key(format!("{unique_id}/server-step-0"))
        .content_type("text/html")
        .send()
        .await
        .unwrap();
    let _ = s3_client
        .put_object()
        .body(s3::primitives::ByteStream::from(Bytes::from(format!(
            "EC2 Client Runner up: {}",
            client_instance_id.clone()
        ))))
        .bucket(STATE.log_bucket)
        .key(format!("{unique_id}/client-step-0"))
        .content_type("text/html")
        .send()
        .await
        .unwrap();

    println!("{:?}", instance_ids);

    let send_command_output_client = send_command("client", &ssm_client, &client_instance_id, vec![
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-1", unique_id).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-2", unique_id).as_str(),
        format!("timeout 1h bash -c 'until yum install cargo git perl openssl-devel bpftrace perf tree -y; do sleep 60; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-3; exit 1)", unique_id).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-3", unique_id).as_str(),
        // format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        "runuser -u ec2-user -- git clone --branch netbench_sync https://github.com/harrisonkaiser/s2n-quic.git",
        format!("runuser -u ec2-user -- echo git finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-4", unique_id).as_str(),
        "runuser -u ec2-user -- aws s3 cp s3://netbenchrunnerlogs/TS/request_response.json /home/ec2-user/request_response.json",
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-5", unique_id).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-6", unique_id).as_str(),
        format!("env SERVER_0={}:4433 COORD_SERVER_0={}:8080 ./scripts/netbench-test-player-as-client.sh", server_ip, server_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-7", unique_id).as_str(),
        "runuser -u ec2-user -- cd target/netbench",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-8", unique_id).as_str(),
        "shutdown -h +1",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out");

    let send_command_output_server = send_command("server", &ssm_client, &server_instance_id, vec![
        format!("runuser -u ec2-user -- echo starting > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-1", unique_id).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-2", unique_id).as_str(),
        format!("timeout 1h bash -c 'until yum install cargo git perl openssl-devel bpftrace perf tree -y; do sleep 60; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-3; exit 1)", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo yum install finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-3", unique_id).as_str(),
        // format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        "runuser -u ec2-user -- git clone --branch netbench_sync https://github.com/harrisonkaiser/s2n-quic.git",
        format!("runuser -u ec2-user -- echo git clone finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-4", unique_id).as_str(),
        "runuser -u ec2-user -- aws s3 cp s3://netbenchrunnerlogs/TS/request_response.json /home/ec2-user/request_response.json",
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-5", unique_id).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-6", unique_id).as_str(),
        format!("env COORD_CLIENT_0={}:8080 ./scripts/netbench-test-player-as-server.sh", client_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-7", unique_id).as_str(),
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-8", unique_id).as_str(),
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out");
    let ssm_command_result_client = wait_for_ssm_results(
        "client",
        &ssm_client,
        send_command_output_client
            .command()
            .unwrap()
            .command_id()
            .unwrap()
    )
    .await;
    println!(
        "Client Finished!: Successful: {}",
        ssm_command_result_client
    );
    let ssm_command_result_server = wait_for_ssm_results(
        "server",
        &ssm_client,
        send_command_output_server
            .command()
            .unwrap()
            .command_id()
            .unwrap()
    )
    .await;
    println!(
        "Server Finished!: Successful: {}",
        ssm_command_result_server
    );

    /*
     * Copy results back
     */
    let generate_report = dbg!(send_command("server", &ssm_client, &server_instance_id, vec![
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/before-sync",
        format!("runuser -u ec2-user -- aws s3 sync s3://netbenchrunnerlogs/{} /home/ec2-user/s2n-quic/netbench/target/netbench", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync",
        "cd /home/ec2-user/s2n-quic/netbench/",
        "runuser -u ec2-user -- ./target/release/netbench-cli report-tree ./target/netbench/results ./target/netbench/report",
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-report",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync-back",
        format!(r#"runuser -u ec2-user -- echo \<a href=\"http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html\"\>Final Report\</a\> > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/finished-step-0"#, unique_id, unique_id).as_str(),
        "shutdown -h +1",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out"));
    let report_result = wait_for_ssm_results(
        "server",
        &ssm_client,
        generate_report
            .command()
            .unwrap()
            .command_id()
            .unwrap()
    )
    .await;

    println!("Report Finished!: Successful: {}", report_result);
    println!(
        "URL: http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html",
        unique_id
    );

    println!("Start: deleting security groups");
    let mut deleted_sec_group = ec2_vpc
        .delete_security_group()
        .group_id(security_group_id.clone())
        .send()
        .await;
    sleep(Duration::from_secs(60));

    while deleted_sec_group.is_err() {
        sleep(Duration::from_secs(30));
        deleted_sec_group = ec2_vpc
            .delete_security_group()
            .group_id(security_group_id.clone())
            .send()
            .await;
    }
    println!("Deleted Security Group: {:#?}", deleted_sec_group);
    println!("Done: deleting security groups");

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
    let launch_template_name = get_launch_template_name(ec2_client, name).await?;
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
