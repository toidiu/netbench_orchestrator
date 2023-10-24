/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
use aws_sdk_ec2 as ec2;
use aws_sdk_ec2::types::InstanceStateName;
use aws_sdk_iam as iam;
use aws_sdk_s3 as s3;
use aws_sdk_ssm as ssm;
use aws_types::region::Region;
use bytes::Bytes;
use s3::primitives::ByteStream;
use std::process::Command;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
};
use std::{thread::sleep, time::Duration};
mod ec2_utils;
mod execute_on_host;
mod s3_helper;
mod state;
mod utils;

use ec2_utils::*;
use execute_on_host::*;
use s3_helper::*;
use state::*;
use utils::*;

fn lines_from_file(filename: impl AsRef<Path>) -> io::Result<Vec<String>> {
    BufReader::new(File::open(filename)?).lines().collect()
}

fn check_requirements() -> Result<(), String> {
    // export PATH="/home/toidiu/projects/s2n-quic/netbench/target/release/:$PATH"
    Command::new("netbench-cli").output().expect(
        "include netbench-cli on PATH: 'export PATH=\"s2n-quic/netbench/target/release/:$PATH\"'",
    );

    // report folder
    std::fs::create_dir_all(STATE.workspace_dir).unwrap();

    // TODO check aws creds

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt::init();

    check_requirements()?;

    let orch_provider = Region::new(STATE.region);
    let shared_config = aws_config::from_env().region(orch_provider).load().await;
    let iam_client = iam::Client::new(&shared_config);
    let s3_client = s3::Client::new(&shared_config);
    let orch_provider_vpc = Region::new(STATE.vpc_region);
    let shared_config_vpc = aws_config::from_env()
        .region(orch_provider_vpc)
        .load()
        .await;
    let ec2_client = ec2::Client::new(&shared_config_vpc);
    let ssm_client = ssm::Client::new(&shared_config_vpc);

    let unique_id = format!(
        "{}-{}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
        STATE.version
    );

    let status = format!("{}/index.html", STATE.cf_url_with_id(&unique_id));
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

    upload_object(
        &s3_client,
        STATE.log_bucket,
        ByteStream::from(Bytes::from(index_file)),
        &format!("{unique_id}/index.html"),
    )
    .await
    .unwrap();

    println!("Status: URL: {status}");

    let instance_details =
        InstanceDetails::new(&unique_id, &ec2_client, &iam_client, &ssm_client).await;
    let server = launch_instance(
        &ec2_client,
        &instance_details,
        format!("server-{}", unique_id).as_str(),
    )
    .await?;

    let client = launch_instance(
        &ec2_client,
        &instance_details,
        format!("client-{}", unique_id).as_str(),
    )
    .await?;

    // Wait for running state
    let mut client_code = InstanceStateName::Pending;
    let mut ip_client = None;
    while dbg!(client_code != InstanceStateName::Running) {
        sleep(Duration::from_secs(30));
        let result = ec2_client
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
        let result = ec2_client
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

    // Modify Security Group
    let client_ip: String = ip_client.unwrap();
    println!("client ip: {}", client_ip);
    let server_ip: String = ip_server.unwrap();
    println!("server ip: {}", server_ip);

    let _network_perms = ec2_client
        .authorize_security_group_egress()
        .group_id(&instance_details.security_group_id)
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
    let _network_perms = ec2_client
        .authorize_security_group_ingress()
        .group_id(&instance_details.security_group_id)
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

    // Setup instances
    let client_instance_id = client
        .instance_id()
        .map(String::from)
        .ok_or(String::from("No client id"))?;
    let server_instance_id = server
        .instance_id()
        .map(String::from)
        .ok_or(String::from("No server id"))?;
    println!(
        "client: {} server: {}",
        client_instance_id, server_instance_id
    );

    upload_object(
        &s3_client,
        STATE.log_bucket,
        ByteStream::from(Bytes::from(format!(
            "EC2 Server Runner up: {} {}",
            server_instance_id, server_ip
        ))),
        &format!("{unique_id}/server-step-0"),
    )
    .await
    .unwrap();

    upload_object(
        &s3_client,
        STATE.log_bucket,
        ByteStream::from(Bytes::from(format!(
            "EC2 Client Runner up: {} {}",
            client_instance_id.clone(),
            client_ip
        ))),
        &format!("{unique_id}/client-step-0"),
    )
    .await
    .unwrap();

    let client_output =
        execute_ssm_client(&ssm_client, client_instance_id, &server_ip, &unique_id).await;
    let server_output =
        execute_ssm_server(&ssm_client, &server_instance_id, &client_ip, &unique_id).await;

    let client_result = wait_for_ssm_results(
        "client",
        &ssm_client,
        client_output.command().unwrap().command_id().unwrap(),
    )
    .await;
    println!("Client Finished!: Successful: {}", client_result);
    let server_result = wait_for_ssm_results(
        "server",
        &ssm_client,
        server_output.command().unwrap().command_id().unwrap(),
    )
    .await;
    println!("Server Finished!: Successful: {}", server_result);

    // Copy results back
    orch_generate_report(&s3_client, &unique_id).await;

    delete_security_group(ec2_client, &instance_details.security_group_id).await;

    Ok(())
}
