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
use tracing::{debug, trace};

pub mod client;
pub mod common;
pub mod server;

pub enum Step {
    Configure,
    BuildRussula,
    BuildNetbench,
    RunRussula,
    RunNetbench,
}

impl Step {
    fn as_str(&self) -> &str {
        match self {
            Step::Configure => "configure",
            Step::BuildRussula => "build_russula",
            Step::BuildNetbench => "build_netbench",
            Step::RunRussula => "run_russula",
            Step::RunNetbench => "run_netbench",
        }
    }
}

pub async fn send_command(
    wait_steps: Vec<Step>,
    step: Step,
    endpoint: &str,
    comment: &str,
    ssm_client: &aws_sdk_ssm::Client,
    ids: Vec<String>,
    commands: Vec<String>,
) -> Option<SendCommandOutput> {
    let mut assemble_command = Vec::new();
    // wait for previous steps
    for step in wait_steps {
        assemble_command.push(format!(
            "cd /home/ec2-user; until [ -f {}_fin___ ]; do sleep 5; done",
            step.as_str()
        ));
    }
    // indicate that this step has started
    assemble_command.push(format!(
        "cd /home/ec2-user; touch {}_start___",
        step.as_str()
    ));
    assemble_command.extend(commands);
    // indicate that this step has finished
    assemble_command.extend(vec![
        "cd /home/ec2-user".to_string(),
        format!("touch {}_fin___", step.as_str()),
    ]);
    debug!("{} {:?}", endpoint, assemble_command);

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
            .parameters("commands", assemble_command.clone())
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
                "endpoint: {} status: {:?}  comment {}",
                endpoint, status, comment
            );

            status
        }
        None => {
            return Ok(Poll::Ready(()));
        }
    };
    trace!("endpoint: {}  command_id {}", endpoint, command_id);

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
