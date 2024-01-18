// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::net::SocketAddr;
use tracing::debug;

pub async fn copy_netbench_data(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    _client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(
        vec![Step::BuildDriver, Step::RunRussula],
        Step::RunNetbench,
        "client",
        "run_client_netbench",
        ssm_client,
        instance_ids,
        vec![
            "cd netbench_orchestrator",
            format!(
                "aws s3 cp server.json {}/results/request_response/s2n-quic/",
                STATE.s3_path(unique_id)
            )
            .as_str(),
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    )
    .await
    .expect("Timed out")
}

pub async fn run_russula_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    peer_sock_addr: SocketAddr,
    netbench_driver: String,
) -> SendCommandOutput {
    debug!("{}", peer_sock_addr);
    send_command(vec![Step::BuildRussula], Step::RunRussula, "server", "run_server_russula", ssm_client, instance_ids, vec![
        "cd netbench_orchestrator",
        format!("env RUST_LOG=debug ./target/debug/russula_cli --russula-port {} netbench-server-worker --peer-list {} --driver {}",
            STATE.russula_port, peer_sock_addr, netbench_driver).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}
