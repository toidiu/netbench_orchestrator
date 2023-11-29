// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;

mod common;

pub use common::{poll_ssm_results, wait_for_ssm_results};

pub async fn configure_client(
    host_group: &str,
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
    unique_id: &str,
) -> SendCommandOutput {
    common::send_command(host_group, "configure_client",ssm_client, instance_id, vec![
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

pub async fn build_client_russula(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
) -> SendCommandOutput {
    common::send_command(
        "client",
        "build_client_russula",
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

pub async fn run_client_russula(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
) -> SendCommandOutput {
    common::send_command("client", "run_client_russula", ssm_client, instance_id, vec![
        // russula START
        "cd /home/ec2-user",
        "until [ -f russula_build_fin ]; do sleep 5; done",
        "sleep 5",
        "touch russula_start----------",
        // format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.russula_branch, STATE.russula_repo).as_str(),
        "cd netbench_orchestrator",
        // "runuser -u ec2-user -- cargo build",
        format!("env RUST_LOG=debug ./target/debug/russula --protocol NetbenchClientWorker --port {}", STATE.russula_port).as_str(),
        // russula END
        // log
        "cd /home/ec2-user",
        "touch russula_run_fin",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn run_client_netbench(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
    server_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    common::send_command("client", "run_client_netbench", ssm_client, instance_id, vec![
        "cd /home/ec2-user",
        "until [ -f russula_run_fin ]; do sleep 5; done",
        "sleep 5",
        "touch run_start----------",
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.netbench_branch, STATE.netbench_repo).as_str(),
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

        // FIXME move to start of ssm commands
        "shutdown -h +60",
        // log
        "cd /home/ec2-user",
        "touch run_fin",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn execute_ssm_server(
    ssm_client: &aws_sdk_ssm::Client,
    instance_id: &str,
    client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    common::send_command("server", "execute_ssm_server", ssm_client, instance_id, vec![
        "cd /home/ec2-user",
        "touch run_start----------",
        format!("runuser -u ec2-user -- echo starting > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-1", STATE.s3_path(unique_id)).as_str(),
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-2", STATE.s3_path(unique_id)).as_str(),
        format!("timeout 5m bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 10; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html {}/server-step-3; exit 1)", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo yum install finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-3", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.netbench_branch, STATE.netbench_repo).as_str(),
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
        "touch run_fin",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}
