// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use aws_sdk_ssm::types::{CloudWatchOutputConfig, CommandInvocationStatus};
use std::thread::sleep;
use std::time::Duration;

pub async fn execute_ssm_client(
    ssm_client: &aws_sdk_ssm::Client,
    client_instance_id: &str,
    server_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("client", ssm_client, client_instance_id, vec![
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-1", STATE.s3_path(unique_id)).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-2", STATE.s3_path(unique_id)).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/client-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-3", STATE.s3_path(unique_id)).as_str(),
        // russula START
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.russula_branch, STATE.russula_repo).as_str(),
        "cd netbench_orchestrator",
        "runuser -u ec2-user -- cargo build",
        format!("runuser -u ec2-user -- RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchClientWorker --port {}", STATE.russula_port).as_str(),
        "touch finished_running1",
        format!("runuser -u ec2-user -- RUST_LOG=debug cargo run --bin russula -- --protocol NetbenchClientWorker --port {}&", STATE.russula_port).as_str(),
        "touch finished_running2",
        "cd ..",

        // russula END
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        format!("runuser -u ec2-user -- echo git finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-4", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- aws s3 cp s3://{}/{}/request_response.json /home/ec2-user/request_response.json", STATE.s3_log_bucket, STATE.s3_resource_folder).as_str(),
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-5", STATE.s3_path(unique_id)).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-6", STATE.s3_path(unique_id)).as_str(),
        format!("env SERVER_0={}:4433 COORD_SERVER_0={}:8080 ./scripts/netbench-test-player-as-client.sh", server_ip, server_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-7", STATE.s3_path(unique_id)).as_str(),
        "runuser -u ec2-user -- cd target/netbench",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench {}", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-8", STATE.s3_path(unique_id)).as_str(),
        "shutdown -h +1",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn execute_ssm_server(
    ssm_client: &aws_sdk_ssm::Client,
    server_instance_id: &str,
    client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("server", ssm_client, server_instance_id, vec![
        format!("runuser -u ec2-user -- echo starting > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-1", STATE.s3_path(unique_id)).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-2", STATE.s3_path(unique_id)).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/server-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo yum install finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-3", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        format!("runuser -u ec2-user -- echo git clone finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-4", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- aws s3 cp s3://{}/{}/request_response.json /home/ec2-user/request_response.json", STATE.s3_log_bucket, STATE.s3_resource_folder).as_str(),
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-5", STATE.s3_path(unique_id)).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-6", STATE.s3_path(unique_id)).as_str(),
        format!("env COORD_CLIENT_0={}:8080 ./scripts/netbench-test-player-as-server.sh", client_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-7", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench {}", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-8", STATE.s3_path(unique_id)).as_str(),
        "shutdown -h +1",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn wait_for_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> bool {
    loop {
        let o_status = ssm_client
            .list_command_invocations()
            .command_id(command_id)
            .send()
            .await
            .unwrap()
            .command_invocations()
            .unwrap()
            .iter()
            .find_map(|command| command.status())
            .cloned();
        let status = match o_status {
            Some(s) => s,
            None => return true,
        };
        let dbg = format!("endpoint: {} status: {:?}", endpoint, status.clone());
        dbg!(dbg);

        match status {
            CommandInvocationStatus::Cancelled
            | CommandInvocationStatus::Cancelling
            | CommandInvocationStatus::Failed
            | CommandInvocationStatus::TimedOut => break false,
            CommandInvocationStatus::Delayed
            | CommandInvocationStatus::InProgress
            | CommandInvocationStatus::Pending => {
                sleep(Duration::from_secs(20));
                continue;
            }
            CommandInvocationStatus::Success => break true,
            _ => panic!("Unhandled Status"),
        };
    }
}

pub async fn send_command(
    _endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
    commands: Vec<String>,
) -> Option<SendCommandOutput> {
    let mut remaining_try_count: u32 = 30;
    loop {
        match ssm_client
            .send_command()
            .instance_ids(instance_id)
            .document_name("AWS-RunShellScript")
            .document_version("$LATEST")
            .parameters("commands", commands.clone())
            .cloud_watch_output_config(
                CloudWatchOutputConfig::builder()
                    .cloud_watch_log_group_name(STATE.cloud_watch_group)
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
                // TODO is this necessary?
                if remaining_try_count > 0 {
                    println!("Error message: {}", error_message);
                    println!("Trying again, waiting 30 seconds...");
                    sleep(Duration::from_secs(10));
                    remaining_try_count -= 1;
                    continue;
                } else {
                    return None;
                }
            }
        };
    }
}
