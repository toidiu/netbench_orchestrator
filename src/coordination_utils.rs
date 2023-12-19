// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::InfraDetail,
    poll_ssm_results, russula,
    russula::{netbench::client, RussulaBuilder},
    ssm_utils::{send_command, Step}, STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use std::{net::SocketAddr, str::FromStr};
use tracing::{debug, info};

pub struct ClientNetbenchRussula {
    worker: SendCommandOutput,
    coord: russula::Russula<client::CoordProtocol>,
}

impl ClientNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        instance_ids: Vec<String>,
    ) -> Self {
        // client run commands
        debug!("starting client worker");
        let worker = client_worker(ssm_client, instance_ids).await;

        // client coord
        debug!("starting client coordinator");
        let coord = client_coord(infra).await;
        ClientNetbenchRussula { worker, coord }
    }

    pub async fn wait_complete(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        // poll client russula workers/coord
        loop {
            let poll_worker = poll_ssm_results(
                "client",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_done = self
                .coord
                .poll_state(client::CoordState::Done)
                .await
                .unwrap();

            debug!(
                "Client Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            if poll_coord_done.is_ready() && poll_worker.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(20)).await;
        }

        info!("Client Russula!: Successful");
    }
}

async fn client_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
) -> SendCommandOutput {
    send_command(vec![Step::BuildRussula], Step::RunRussula, "client", "run_client_russula", ssm_client, instance_ids, vec![
        "cd netbench_orchestrator",
        format!("env RUST_LOG=debug ./target/debug/russula --protocol NetbenchClientWorker --port {}", STATE.russula_port).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

async fn client_coord(infra: &InfraDetail) -> russula::Russula<client::CoordProtocol> {
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
