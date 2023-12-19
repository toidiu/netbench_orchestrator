// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::InfraDetail,
    poll_ssm_results, russula,
    russula::{
        netbench::{client, server},
        RussulaBuilder,
    },
    ssm_utils, STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use std::{net::SocketAddr, str::FromStr};
use tracing::{debug, info};

pub struct ServerNetbenchRussula {
    worker: SendCommandOutput,
    coord: russula::Russula<server::CoordProtocol>,
}

impl ServerNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        instance_ids: Vec<String>,
    ) -> Self {
        // server run commands
        debug!("starting server worker");
        let worker = ssm_utils::server::run_russula_worker(ssm_client, instance_ids).await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(10)).await;

        // server coord
        debug!("starting server coordinator");
        let coord = server_coord(infra).await;
        ServerNetbenchRussula { worker, coord }
    }

    pub async fn wait_workers_running(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        loop {
            let poll_worker = poll_ssm_results(
                "server",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_worker_running = self
                .coord
                .poll_state(server::CoordState::WorkersRunning)
                .await
                .unwrap();

            debug!(
                "Server Russula!: poll worker_running. Coordinator: {:?} Worker {:?}",
                poll_coord_worker_running, poll_worker
            );

            if poll_coord_worker_running.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    // FIXME de-dupe with other wait_done fn on Client
    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        // poll server russula workers/coord
        loop {
            let poll_worker = poll_ssm_results(
                "server",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_done = self.coord.poll_done().await.unwrap();

            debug!(
                "Server Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            if poll_coord_done.is_ready() && poll_worker.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        info!("Server Russula!: Successful");
    }
}

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
        let worker = ssm_utils::client::run_russula_worker(ssm_client, instance_ids).await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(10)).await;

        // client coord
        debug!("starting client coordinator");
        let coord = client_coord(infra).await;
        ClientNetbenchRussula { worker, coord }
    }

    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        // poll client russula workers/coord
        loop {
            let poll_worker = poll_ssm_results(
                "client",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_done = self.coord.poll_done().await.unwrap();

            debug!(
                "Client Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            if poll_coord_done.is_ready() && poll_worker.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }

        info!("Client Russula!: Successful");
    }
}

async fn server_coord(infra: &InfraDetail) -> russula::Russula<server::CoordProtocol> {
    let server_ips = infra
        .servers
        .iter()
        .map(|instance| {
            SocketAddr::from_str(&format!("{}:{}", instance.ip, STATE.russula_port)).unwrap()
        })
        .collect();
    let server_coord = RussulaBuilder::new(server_ips, server::CoordProtocol::new());
    let mut server_coord = server_coord.build().await.unwrap();
    server_coord.run_till_ready().await.unwrap();
    info!("server coord Ready");
    server_coord
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
    client_coord.run_till_ready().await.unwrap();
    info!("client coord Ready");
    client_coord
}
