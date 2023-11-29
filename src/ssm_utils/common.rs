// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::send_command;
use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;

pub async fn configure_hosts(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: Vec<String>,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(host_group, "configure_client",ssm_client, instance_id, vec![
        "cd /home/ec2-user",
        "touch config_start----------",
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-1", STATE.s3_path(unique_id), host_group).as_str(),
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-2", STATE.s3_path(unique_id), host_group).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/server-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/{}-step-3", STATE.s3_path(unique_id), host_group).as_str(),
        // log
        "cd /home/ec2-user",
        "touch config_fin",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn build_russula(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: Vec<String>,
) -> SendCommandOutput {
    send_command(
        "client",
        "build_russula",
        ssm_client,
        instance_id,
        vec![
            // russula START
            "cd /home/ec2-user",
            "until [ -f config_fin ]; do sleep 5; done",
            "sleep 5",
            "touch russula_build_start----------",
            format!(
                "runuser -u ec2-user -- git clone --branch {} {}",
                STATE.russula_branch, STATE.russula_repo
            )
            .as_str(),
            "cd netbench_orchestrator",
            "runuser -u ec2-user -- cargo build",
            // russula END
            // log
            "cd /home/ec2-user",
            "touch russula_build_fin",
            "exit 0",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    )
    .await
    .expect("Timed out")
}
