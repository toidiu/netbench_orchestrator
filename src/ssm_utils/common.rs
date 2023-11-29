// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::{OrchError, OrchResult},
    state::STATE,
};
use aws_sdk_ssm::{
    operation::send_command::SendCommandOutput,
    types::{CloudWatchOutputConfig, CommandInvocationStatus},
};
use core::task::Poll;
use std::{thread::sleep, time::Duration};
use tracing::debug;

pub(crate) async fn configure_client(
    ssm_client: &aws_sdk_ssm::Client,
    client_instance_id: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("client", "configure_client",ssm_client, client_instance_id, vec![

        "cd /home/ec2-user",
        "touch config_start----------",
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-1", STATE.s3_path(unique_id)).as_str(),
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-2", STATE.s3_path(unique_id)).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/server-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-3", STATE.s3_path(unique_id)).as_str(),
        // log
        "cd /home/ec2-user",
        "touch config_fin",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub(crate) async fn send_command(
    endpoint: &str,
    comment: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
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

pub async fn wait_for_ssm_results(
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

pub async fn poll_ssm_results(
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
