// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use crate::report::orch_generate_report;
use aws_types::region::Region;
use error::{OrchError, OrchResult};
use russula::{netbench::client, RussulaBuilder};
use std::{net::SocketAddr, process::Command, str::FromStr, time::Duration};
use tracing::{debug, info};
mod dashboard;
mod ec2_utils;
mod error;
mod report;
mod russula;
mod s3_utils;
mod ssm_utils;
mod state;

use dashboard::*;
use ec2_utils::*;
use s3_utils::*;
use ssm_utils::*;
use state::*;

// TODO
// D- russula poll state
// D- interleave poll russula and ssm
//
// D- cleanup ssm
// - breakup server ssm
// - ssm take list of ids
//
// - server russula
// - enable cleanup

async fn check_requirements(iam_client: &aws_sdk_iam::Client) -> OrchResult<()> {
    // export PATH="/home/toidiu/projects/s2n-quic/netbench/target/release/:$PATH"
    Command::new("netbench-cli")
        .output()
        .map_err(|_err| OrchError::Init {
            dbg: "Missing netbench-cli. export PATH='.../s2n-quic/netbench/target/release/:$PATH'"
                .to_string(),
        })?;

    // report folder
    std::fs::create_dir_all(STATE.workspace_dir).map_err(|_err| OrchError::Init {
        dbg: "Failed to create local workspace".to_string(),
    })?;

    iam_client
        .list_roles()
        .send()
        .await
        .map_err(|_err| OrchError::Init {
            dbg: "Missing AWS creds".to_string(),
        })?;

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    tracing_subscriber::fmt::init();

    let orch_provider = Region::new(STATE.region);
    let shared_config = aws_config::from_env().region(orch_provider).load().await;
    let iam_client = aws_sdk_iam::Client::new(&shared_config);
    let s3_client = aws_sdk_s3::Client::new(&shared_config);
    let orch_provider_vpc = Region::new(STATE.vpc_region);
    let shared_config_vpc = aws_config::from_env()
        .region(orch_provider_vpc)
        .load()
        .await;
    let ec2_client = aws_sdk_ec2::Client::new(&shared_config_vpc);
    let ssm_client = aws_sdk_ssm::Client::new(&shared_config_vpc);

    check_requirements(&iam_client).await?;

    let unique_id = format!(
        "{}-{}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
        STATE.version
    );

    update_dashboard(Step::UploadIndex, &s3_client, &unique_id).await?;

    // Setup instances
    let infra = LaunchPlan::create(
        &unique_id,
        &ec2_client,
        &iam_client,
        &ssm_client,
        STATE.host_count,
    )
    .await
    .launch(&ec2_client, &unique_id)
    .await?;
    let client = &infra.clients[0];
    let server = &infra.servers[0];
    let client_ids: Vec<String> = infra
        .clients
        .clone()
        .into_iter()
        .map(|infra_detail| {
            let id = infra_detail.instance_id().unwrap();
            id.to_string()
        })
        .collect();

    let client_instance_id = client.instance_id()?;
    let server_instance_id = server.instance_id()?;

    update_dashboard(
        Step::ServerHostsRunning(&infra.servers),
        &s3_client,
        &unique_id,
    )
    .await?;
    update_dashboard(
        Step::ServerHostsRunning(&infra.clients),
        &s3_client,
        &unique_id,
    )
    .await?;

    // TODO move into ssm_utils
    {
        // server
        let server_output =
            execute_ssm_server(&ssm_client, server_instance_id, &client.ip, &unique_id).await;

        // client
        let configure_client = ssm_utils::common::configure_hosts(
            "client",
            &ssm_client,
            client_ids.clone(),
            &unique_id,
        )
        .await;
        let build_russula =
            ssm_utils::common::build_russula("client", &ssm_client, client_ids.clone()).await;
        let run_client_russula =
            ssm_utils::client::run_client_russula(&ssm_client, client_ids).await;
        let build_client_netbench = ssm_utils::common::build_netbench(
            "client",
            &ssm_client,
            client_instance_id,
            &unique_id,
        )
        .await;
        let run_client_netbench = ssm_utils::client::run_client_netbench(
            &ssm_client,
            client_instance_id,
            &server.ip,
            &unique_id,
        )
        .await;

        // wait complete
        let configure_client = ssm_utils::wait_for_ssm_results(
            "client",
            &ssm_client,
            configure_client.command().unwrap().command_id().unwrap(),
        )
        .await;
        info!("Client Config!: Successful: {}", configure_client);
        // wait complete
        let build_russula = wait_for_ssm_results(
            "client",
            &ssm_client,
            build_russula.command().unwrap().command_id().unwrap(),
        )
        .await;
        info!("Client Russula build!: Successful: {}", build_russula);

        {
            let client_ips = infra
                .clients
                .iter()
                .map(|instance| {
                    SocketAddr::from_str(&format!("{}:{}", instance.ip, STATE.russula_port))
                        .unwrap()
                })
                .collect();
            let client_coord = RussulaBuilder::new(client_ips, client::CoordProtocol::new());
            let mut client_coord = client_coord.build().await.unwrap();
            client_coord.run_till_ready().await;
            info!("client coord Ready");

            // run russula workers
            loop {
                let poll_worker = poll_ssm_results(
                    "server",
                    &ssm_client,
                    run_client_russula.command().unwrap().command_id().unwrap(),
                )
                .await
                .unwrap();

                let poll_coord_done = client_coord
                    .poll_state(client::CoordState::Done)
                    .await
                    .unwrap();

                debug!(
                    "Client Russula!: Coordinator: {:?} Worker {:?}",
                    poll_coord_done, poll_worker
                );

                if poll_coord_done.is_ready() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(10)).await;
            }

            let wait_worker = wait_for_ssm_results(
                "server",
                &ssm_client,
                run_client_russula.command().unwrap().command_id().unwrap(),
            )
            .await;
            info!("Client Russula!: Successful worker: {}", wait_worker);
        }

        let build_client_netbench = wait_for_ssm_results(
            "client",
            &ssm_client,
            build_client_netbench
                .command()
                .unwrap()
                .command_id()
                .unwrap(),
        )
        .await;
        info!(
            "Client build netbench!: Successful: {}",
            build_client_netbench
        );
        let run_client_netbench = wait_for_ssm_results(
            "client",
            &ssm_client,
            run_client_netbench.command().unwrap().command_id().unwrap(),
        )
        .await;
        info!("Client Finished!: Successful: {}", run_client_netbench);

        let server_result = wait_for_ssm_results(
            "server",
            &ssm_client,
            server_output.command().unwrap().command_id().unwrap(),
        )
        .await;
        info!("Server Finished!: Successful: {}", server_result);
    }

    // Copy results back
    orch_generate_report(&s3_client, &unique_id).await;

    // infra
    //     .cleanup(&ec2_client)
    //     .await
    //     .map_err(|err| eprintln!("Failed to cleanup resources. {}", err))
    //     .unwrap();

    Ok(())
}
