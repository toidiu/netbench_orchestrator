// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use crate::report::orch_generate_report;
use aws_types::region::Region;
use error::{OrchError, OrchResult};
use russula::{netbench::client, RussulaBuilder};
use std::{net::SocketAddr, process::Command, str::FromStr};
use tracing::info;
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
//
// - make ssm russula runs nicer
// - get rid of runuser
//
// D- russula poll state
// D- interleave poll russula and ssm
// D- cleanup ssm
// D- breakup server ssm
// D- ssm take list of ids
// D- parallize netbench and russula build
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
    let server_ids: Vec<String> = infra
        .servers
        .clone()
        .into_iter()
        .map(|infra_detail| {
            let id = infra_detail.instance_id().unwrap();
            id.to_string()
        })
        .collect();

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

    // configure and build
    {
        let mut build_cmds = ssm_utils::common::collect_config_cmds(
            "server",
            &ssm_client,
            server_ids.clone(),
            &unique_id,
        )
        .await;
        let client_build_cmds = ssm_utils::common::collect_config_cmds(
            "client",
            &ssm_client,
            client_ids.clone(),
            &unique_id,
        )
        .await;
        build_cmds.extend(client_build_cmds);
        ssm_utils::common::wait_cmds("client_server_config", &ssm_client, build_cmds).await;

        info!("client_server install_deps!: Successful");
    }

    // run russula
    {
        // client run commands
        let client_worker = ssm_utils::client::run_russula(&ssm_client, client_ids.clone()).await;

        // client coord
        let client_coord = {
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
            client_coord
        };

        ssm_utils::client::wait_russula(&ssm_client, client_coord, client_worker).await;
    }

    // run netbench
    {
        let run_server_netbench = ssm_utils::server::run_netbench(
            &ssm_client,
            server_ids.clone(),
            &client.ip,
            &unique_id,
        )
        .await;
        let run_client_netbench = ssm_utils::client::run_netbench(
            &ssm_client,
            client_ids.clone(),
            &server.ip,
            &unique_id,
        )
        .await;
        ssm_utils::common::wait_cmds(
            "client_server_netbench",
            &ssm_client,
            vec![run_server_netbench, run_client_netbench],
        )
        .await;
        info!("client_server netbench!: Successful");
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
