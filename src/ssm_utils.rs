// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::error::{OrchError, OrchResult};
use crate::state::STATE;
use aws_sdk_ssm::{
    operation::send_command::SendCommandOutput,
    types::{CloudWatchOutputConfig, CommandInvocationStatus},
};
use core::task::Poll;
use std::{thread::sleep, time::Duration};
use tracing::debug;

pub mod client;
pub mod common;

pub async fn execute_ssm_server(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
    client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("server", "execute_ssm_server", ssm_client, vec![instance_id.to_string()], vec![
        "cd /home/ec2-user",
        "touch run_start----------",
        format!("runuser -u ec2-user -- echo starting > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-1", STATE.s3_path(unique_id)).as_str(),
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-2", STATE.s3_path(unique_id)).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/server-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo yum install finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-3", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.netbench_branch, STATE.netbench_repo).as_str(),
        format!("runuser -u ec2-user -- echo clone_netbench > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-4", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- aws s3 cp s3://{}/{}/request_response.json /home/ec2-user/request_response.json", STATE.s3_log_bucket, STATE.s3_resource_folder).as_str(),
        format!("runuser -u ec2-user -- echo downloaded_scenario_file > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-5", STATE.s3_path(unique_id)).as_str(),
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
        "touch run_fin",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

async fn send_command(
    endpoint: &str,
    comment: &str,
    ssm_client: &aws_sdk_ssm::Client,
    ids: Vec<String>,
    commands: Vec<String>,
) -> Option<SendCommandOutput> {
    let mut remaining_try_count: u32 = 30;
    loop {
        debug!(
            "send_command... endpoint: {} remaining_try_count: {} comment: {}",
            endpoint, remaining_try_count, comment
        );
        match ssm_client
            .send_command()
            .comment(comment)
            // .instance_ids(ids)
            .set_instance_ids(Some(ids.clone()))
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
            Err(err) => {
                // TODO is this necessary?
                if remaining_try_count > 0 {
                    debug!(
                        "Send command failed: remaining: {} err: {}",
                        remaining_try_count, err
                    );
                    sleep(Duration::from_secs(2));
                    remaining_try_count -= 1;
                    continue;
                } else {
                    return None;
                }
            }
        };
    }
}

pub(crate) async fn wait_for_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> bool {
    loop {
        match poll_ssm_results(endpoint, ssm_client, command_id).await {
            Ok(Poll::Ready(_)) => break true,
            Ok(Poll::Pending) => {
                // FIXME can we use tokio sleep here?
                sleep(Duration::from_secs(10));
                // tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
            Err(_err) => break false,
        }
    }
}

pub(crate) async fn poll_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> OrchResult<Poll<()>> {
    let status_comment = ssm_client
        .list_command_invocations()
        .command_id(command_id)
        .send()
        .await
        .unwrap()
        .command_invocations()
        .unwrap()
        .iter()
        .find_map(|command| {
            let status = command.status().cloned();
            let comment = command.comment().map(|s| s.to_string());
            status.zip(comment)
        });
    let status = match status_comment {
        Some((status, comment)) => {
            debug!(
                "endpoint: {} status: {:?} command_id {}, comment {}",
                endpoint, status, command_id, comment
            );

            status
        }
        None => {
            debug!("{} command complete: {}", endpoint, command_id);
            return Ok(Poll::Ready(()));
        }
    };

    let status = match status {
        CommandInvocationStatus::Cancelled
        | CommandInvocationStatus::Cancelling
        | CommandInvocationStatus::Failed
        | CommandInvocationStatus::TimedOut => {
            return Err(OrchError::Ssm {
                dbg: "timeout".to_string(),
            })
        }
        CommandInvocationStatus::Delayed
        | CommandInvocationStatus::InProgress
        | CommandInvocationStatus::Pending => Poll::Pending,
        CommandInvocationStatus::Success => Poll::Ready(()),
        _ => {
            return Err(OrchError::Ssm {
                dbg: "unhandled status".to_string(),
            })
        }
    };
    Ok(status)
}
