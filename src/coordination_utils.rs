// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::InfraDetail,
    poll_ssm_results,
    russula::{
        self,
        netbench::{client, server},
        RussulaBuilder,
    },
    ssm_utils, NetbenchDriver, Scenario, STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use std::{collections::BTreeSet, net::SocketAddr, str::FromStr};
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
        scenario: &Scenario,
        driver: NetbenchDriver,
    ) -> Self {
        // server run commands
        debug!("starting server worker");

        let worker = ssm_utils::server::run_russula_worker(
            ssm_client,
            instance_ids,
            driver.driver_name,
            scenario,
        )
        .await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(5)).await;

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
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

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

            // FIXME the worker doesnt complete but its not necessary to wait so continue.
            //
            // maybe try sudo
            //
            // The collector launches the driver process, which doesnt get killed when the
            // collector is killed. However its not necessary to wait for its completing
            // for the purpose of a single run.
            // ```
            //  55320  ./target/debug/russula_cli
            //  55646  /home/ec2-user/bin/netbench-collector
            //  55647  /home/ec2-user/bin/netbench-driver-s2n-quic-server
            // ```
            if poll_coord_done.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
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
        server_ips: &[String],
        scenario: &Scenario,
        driver: NetbenchDriver,
    ) -> Self {
        // client run commands
        debug!("starting client worker");

        let peer_sock_addr: Vec<SocketAddr> = server_ips
            .iter()
            .map(|ip| SocketAddr::from_str(&format!("{}:4433", ip)).unwrap())
            .collect();
        let worker = ssm_utils::client::run_russula_worker(
            ssm_client,
            instance_ids,
            peer_sock_addr,
            driver.driver_name,
            scenario,
        )
        .await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(5)).await;

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

            if poll_coord_done.is_ready() {
                // if poll_coord_done.is_ready() && poll_worker.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        info!("Client Russula!: Successful");
    }
}

async fn server_coord(infra: &InfraDetail) -> russula::Russula<server::CoordProtocol> {
    let server_ips: Vec<SocketAddr> = infra
        .servers
        .iter()
        .map(|instance| {
            SocketAddr::from_str(&format!("{}:{}", instance.ip, STATE.russula_port)).unwrap()
        })
        .collect();

    let protocol = server::CoordProtocol::new();
    let server_coord = RussulaBuilder::new(
        BTreeSet::from_iter(server_ips),
        protocol,
        STATE.poll_delay_russula,
    );
    let mut server_coord = server_coord.build().await.unwrap();
    server_coord.run_till_ready().await.unwrap();
    info!("server coord Ready");
    server_coord
}

async fn client_coord(infra: &InfraDetail) -> russula::Russula<client::CoordProtocol> {
    let client_ips: Vec<SocketAddr> = infra
        .clients
        .iter()
        .map(|instance| {
            SocketAddr::from_str(&format!("{}:{}", instance.ip, STATE.russula_port)).unwrap()
        })
        .collect();

    let protocol = client::CoordProtocol::new();
    let client_coord = RussulaBuilder::new(
        BTreeSet::from_iter(client_ips),
        protocol,
        STATE.poll_delay_russula,
    );
    let mut client_coord = client_coord.build().await.unwrap();
    client_coord.run_till_ready().await.unwrap();
    info!("client coord Ready");
    client_coord
}
