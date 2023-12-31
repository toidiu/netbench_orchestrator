// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{poll_ssm_results, state::STATE};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use tracing::debug;

pub async fn wait_complete(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    cmds: Vec<SendCommandOutput>,
) {
    let total_tasks = cmds.len() as u64;
    let bar = ProgressBar::new(total_tasks);
    let style = ProgressStyle::with_template(
        "{spinner} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
    bar.set_style(style);
    bar.enable_steady_tick(Duration::from_secs(1));

    loop {
        let mut completed_tasks = 0;
        for cmd in cmds.iter() {
            let cmd_id = cmd.command().unwrap().command_id().unwrap();
            let poll_cmd = poll_ssm_results(host_group, ssm_client, cmd_id)
                .await
                .unwrap();
            if poll_cmd.is_ready() {
                completed_tasks += 1;
            }
        }

        bar.set_position(completed_tasks);
        bar.set_message(host_group.to_string());

        if total_tasks == completed_tasks {
            // debug!("{} SSM poll complete", host_group);
            bar.finish();
            break;
        } else {
            // debug!("tasks not complete. wait to poll again ...");
        }
        tokio::time::sleep(STATE.poll_cmds_duration).await;
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
        // set instances to shutdown after 1 hour
        format!("shutdown -P +{}", STATE.shutdown_min),
        "mkdir -p /home/ec2-user/bin".to_string(),

        format!("echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-1", STATE.s3_path(unique_id), host_group),
        "yum upgrade -y".to_string(),
        format!("echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-2", STATE.s3_path(unique_id), host_group),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/{}-step-3; exit 1)", STATE.s3_path(unique_id), host_group),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-3", STATE.s3_path(unique_id), host_group),
    ]).await.expect("Timed out")
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
                "git clone --branch {} {}",
                STATE.russula_branch, STATE.russula_repo
            )
            .as_str(),
            "cd netbench_orchestrator",
            "cargo build",
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
        format!("git clone --branch {} {}", STATE.netbench_branch, STATE.netbench_repo),
        format!("echo clone_netbench > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-4", STATE.s3_path(unique_id), host_group),
        format!("aws s3 cp s3://{}/{}/request_response.json /home/ec2-user/request_response.json", STATE.s3_log_bucket, STATE.s3_resource_folder),
        format!("echo downloaded_scenario_file > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-5", STATE.s3_path(unique_id), host_group),

        // FIXME enable this
        // format!("aws s3 sync s3://{}/{}/bin /home/ec2-user/bin", STATE.s3_log_bucket, STATE.s3_resource_folder),
        // "sudo chmod +x /home/ec2-user/bin/*".to_string(),

        "cd s2n-quic/netbench".to_string(),
        "cargo build --release".to_string(),
        // copy netbench executables to ~/bin folder
        "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {} /home/ec2-user/bin \\;".to_string(),

        "mkdir -p target/netbench".to_string(),
        "cp /home/ec2-user/request_response.json target/netbench/request_response.json".to_string(),
        format!("echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-6", STATE.s3_path(unique_id), host_group),
    ]).await.expect("Timed out")
}
