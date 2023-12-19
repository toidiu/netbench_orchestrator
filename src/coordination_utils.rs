// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::InfraDetail,
    russula,
    russula::{netbench::client, RussulaBuilder},
    ssm_utils::{send_command, Step},
    STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::{net::SocketAddr, str::FromStr};
use tracing::info;

pub async fn client_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
) -> SendCommandOutput {
    send_command(vec![Step::BuildRussula], Step::RunRussula, "client", "run_client_russula", ssm_client, instance_ids, vec![
        "cd netbench_orchestrator",
        format!("env RUST_LOG=debug ./target/debug/russula --protocol NetbenchClientWorker --port {}", STATE.russula_port).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn client_coord(infra: &InfraDetail) -> russula::Russula<client::CoordProtocol> {
    let client_ips = infra
        .clients
        .iter()
        .map(|instance| {
            SocketAddr::from_str(&format!("{}:{}", instance.ip, STATE.russula_port)).unwrap()
        })
        .collect();
    let client_coord = RussulaBuilder::new(client_ips, client::CoordProtocol::new());
    let mut client_coord = client_coord.build().await.unwrap();
    client_coord.run_till_ready().await;
    info!("client coord Ready");
    client_coord
}
