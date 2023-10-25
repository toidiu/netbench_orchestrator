/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(dead_code)]
use crate::report::orch_generate_report;
use aws_sdk_s3::primitives::ByteStream;
use aws_types::region::Region;
use bytes::Bytes;
use std::process::Command;
mod ec2_utils;
mod error;
mod report;
mod s3_utils;
mod ssm_utils;
mod state;

use ec2_utils::*;
use s3_utils::*;
use ssm_utils::*;
use state::*;

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

    // Setup instances
    let launch_plan = LaunchPlan::create(&unique_id, &ec2_client, &iam_client, &ssm_client).await;
    let infra = launch_plan.launch(&ec2_client, &unique_id).await.unwrap();
    let client = infra.clients.get(0).unwrap();
    let server = infra.server.get(0).unwrap();

    let client_instance_id = client.instance_id().unwrap();
    let server_instance_id = server.instance_id().unwrap();

    // TODO move elsewhere update status
    {
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
    }

    // TODO move into ssm_utils
    {
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
    }

    // Copy results back
    orch_generate_report(&s3_client, &unique_id).await;

    infra.cleanup(&ec2_client).await;

    Ok(())
}
