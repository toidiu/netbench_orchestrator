// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{poll_ssm_results, state::STATE};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use tracing::debug;

pub async fn wait_cmds(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    cmds: Vec<SendCommandOutput>,
) {
    loop {
        let mut complete = true;
        for cmd in cmds.iter() {
            let cmd_id = cmd.command().unwrap().command_id().unwrap();
            let poll_cmd = poll_ssm_results(host_group, ssm_client, cmd_id)
                .await
                .unwrap();
            complete &= poll_cmd.is_ready();
        }

        if complete {
            debug!("{} SSM poll complete", host_group);
            break;
        } else {
            debug!("tasks not complete. wait to poll  again ...");
        }
        tokio::time::sleep(Duration::from_secs(20)).await;
    }
}

pub async fn collect_config_cmds(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    unique_id: &str,
) -> Vec<SendCommandOutput> {
    // configure and build
    let install_deps =
        install_deps_cmd(host_group, ssm_client, instance_ids.clone(), unique_id).await;
    let build_russula = build_russula_cmd(host_group, ssm_client, instance_ids.clone()).await;
    let build_client_netbench =
        build_netbench_cmd(host_group, ssm_client, instance_ids.clone(), unique_id).await;
    vec![install_deps, build_russula, build_client_netbench]
}

async fn install_deps_cmd(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(vec![], Step::Configure, host_group, &format!("configure_host_{}", host_group) ,ssm_client, instance_ids, vec![
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-1", STATE.s3_path(unique_id), host_group).as_str(),
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-2", STATE.s3_path(unique_id), host_group).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/{}-step-3; exit 1)", STATE.s3_path(unique_id), host_group).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-3", STATE.s3_path(unique_id), host_group).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

async fn build_russula_cmd(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
) -> SendCommandOutput {
    send_command(
        vec![Step::Configure],
        Step::BuildRussula,
        host_group,
        &format!("build_russula_{}", host_group),
        ssm_client,
        instance_ids,
        vec![
            format!(
                "runuser -u ec2-user -- git clone --branch {} {}",
                STATE.russula_branch, STATE.russula_repo
            )
            .as_str(),
            "cd netbench_orchestrator",
            "runuser -u ec2-user -- cargo build",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    )
    .await
    .expect("Timed out")
}

async fn build_netbench_cmd(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(
        vec![Step::Configure],
        Step::BuildNetbench,
        host_group,
        &format!("build_netbench_{}", host_group),
        ssm_client, instance_ids,
        vec![
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.netbench_branch, STATE.netbench_repo).as_str(),
        format!("runuser -u ec2-user -- echo clone_netbench > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-4", STATE.s3_path(unique_id), host_group).as_str(),
        format!("runuser -u ec2-user -- aws s3 cp s3://{}/{}/request_response.json /home/ec2-user/request_response.json", STATE.s3_log_bucket, STATE.s3_resource_folder).as_str(),
        format!("runuser -u ec2-user -- echo downloaded_scenario_file > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-5", STATE.s3_path(unique_id), host_group).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-6", STATE.s3_path(unique_id), host_group).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}
