/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
use aws_sdk_s3::primitives::ByteStream;
use aws_types::region::Region;
use bytes::Bytes;
use std::process::Command;
mod ec2_utils;
mod error;
mod execute_on_host;
mod s3_helper;
mod state;
mod utils;

use ec2_utils::*;
use execute_on_host::*;
use s3_helper::*;
use state::*;
use utils::*;

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
    let iam_client = aws_sdk_iam::Client::new(&shared_config);
    let s3_client = aws_sdk_s3::Client::new(&shared_config);
    let orch_provider_vpc = Region::new(STATE.vpc_region);
    let shared_config_vpc = aws_config::from_env()
        .region(orch_provider_vpc)
        .load()
        .await;
    let ec2_client = aws_sdk_ec2::Client::new(&shared_config_vpc);
    let ssm_client = aws_sdk_ssm::Client::new(&shared_config_vpc);

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

    let instance_details = LaunchPlan::new(&unique_id, &ec2_client, &iam_client, &ssm_client).await;
    let (server, client) = launch_server_client(&ec2_client, &instance_details, &unique_id)
        .await
        .unwrap();

    // Modify Security Group
    println!("client ip: {}", client.ip);
    println!("server ip: {}", server.ip);

    let _network_perms = ec2_client
        .authorize_security_group_egress()
        .group_id(&instance_details.security_group_id)
        .ip_permissions(
            aws_sdk_ec2::types::IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", client.ip))
                        .build(),
                )
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", server.ip))
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
            aws_sdk_ec2::types::IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", client.ip))
                        .build(),
                )
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", server.ip))
                        .build(),
                )
                .build(),
        )
        .ip_permissions(
            aws_sdk_ec2::types::IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip("0.0.0.0/0")
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .expect("error");

    // Setup instances
    let client_instance_id = client.instance_id().unwrap();
    let server_instance_id = server.instance_id().unwrap();

    upload_object(
        &s3_client,
        STATE.log_bucket,
        ByteStream::from(Bytes::from(format!(
            "EC2 Server Runner up: {} {}",
            server_instance_id, server.ip
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
            client_instance_id, client.ip
        ))),
        &format!("{unique_id}/client-step-0"),
    )
    .await
    .unwrap();

    let client_output =
        execute_ssm_client(&ssm_client, client_instance_id, &server.ip, &unique_id).await;
    let server_output =
        execute_ssm_server(&ssm_client, server_instance_id, &client.ip, &unique_id).await;

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
