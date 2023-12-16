// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;

pub async fn run_client_russula(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
) -> SendCommandOutput {
    send_command(vec![Step::BuildRussula], Step::RunRussula, "client", "run_client_russula", ssm_client, instance_ids, vec![
        "cd netbench_orchestrator",
        format!("env RUST_LOG=debug ./target/debug/russula --protocol NetbenchClientWorker --port {}", STATE.russula_port).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn run_client_netbench(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    server_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(vec![ Step::BuildNetbench, Step::RunRussula], Step::RunNetbench, "client", "run_client_netbench", ssm_client, instance_ids, vec![
        "cd s2n-quic/netbench",
        format!("env SERVER_0={}:4433 COORD_SERVER_0={}:8080 ./scripts/netbench-test-player-as-client.sh", server_ip, server_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-7", STATE.s3_path(unique_id)).as_str(),
        "runuser -u ec2-user -- cd target/netbench",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench {}", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/client-step-8", STATE.s3_path(unique_id)).as_str(),

        // FIXME move to start of ssm commands
        "shutdown -h +60",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}