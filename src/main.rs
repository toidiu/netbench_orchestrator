// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use crate::common::CustomDriver;
use crate::report::orch_generate_report;
use aws_types::region::Region;
use error::{OrchError, OrchResult};
use std::process::{Command, Stdio};
use tracing::debug;
use tracing::info;

mod coordination_utils;
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
// D- upload source to s3
// - download source from s3
// - navigate to source and compile
//
// - specify scenario, driver and path to netbench in russula_cli
//   - pass russula_cli args down to russula
//
// - experiment with uploading and downloading netbench exec
// - experiment with uploading and downloading russula exec

async fn check_requirements(iam_client: &aws_sdk_iam::Client) -> OrchResult<()> {
    // export PATH="/home/toidiu/projects/s2n-quic/netbench/target/release/:$PATH"
    Command::new("s2n-netbench")
        .output()
        .map_err(|_err| OrchError::Init {
            dbg: "Missing `s2n-netbench` cli. Please the Getting started section in the Readme"
                .to_string(),
        })?;

    Command::new("aws")
        .output()
        .map_err(|_err| OrchError::Init {
            dbg: "Missing `aws` cli.".to_string(),
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
            dbg: "Missing AWS credentials.".to_string(),
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

    // custom driver
    let driver = {
        let driver = CustomDriver {
            name: "SaltyLib-Rust".into(),
            local_path: "/Users/apoorvko/projects/ws_SaltyLib/src".into(),

            // TODO
            // curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
            build_cmd: vec![
                // SSM agent doesn't pick up the newest rustc version installed via rustup`
                // so instead refer to it directly
                "env RUSTFLAGS='--cfg s2n_quic_unstable' /home/ec2-user/.cargo/bin/cargo build".to_string(),
                // copy executables to bin directory
                "find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {} /home/ec2-user/bin \\;".to_string(),
            ]
        };

        let mut local_to_s3_cmd = Command::new("aws");
        local_to_s3_cmd.args(["s3", "sync"]).stdout(Stdio::null());
        local_to_s3_cmd
            .arg(format!(
                "{}/{}",
                driver.local_path.to_str().unwrap(),
                driver.name
            ))
            .arg(format!("{}/{}/", STATE.s3_path(&unique_id), driver.name));
        local_to_s3_cmd.args(["--exclude", "target/*", "--exclude", ".git/*"]);

        debug!("{:?}", local_to_s3_cmd);

        let status = local_to_s3_cmd.status().unwrap();
        assert!(status.success(), "aws sync command failed");
        driver
    };

    update_dashboard(dashboard::Step::UploadIndex, &s3_client, &unique_id).await?;

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
        dashboard::Step::ServerHostsRunning(&infra.servers),
        &s3_client,
        &unique_id,
    )
    .await?;
    update_dashboard(
        dashboard::Step::ServerHostsRunning(&infra.clients),
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
            &driver,
            &unique_id,
        )
        .await;
        let client_build_cmds = ssm_utils::common::collect_config_cmds(
            "client",
            &ssm_client,
            client_ids.clone(),
            &driver,
            &unique_id,
        )
        .await;
        build_cmds.extend(client_build_cmds);
        ssm_utils::common::wait_complete(
            "Setup hosts: update and install dependencies",
            &ssm_client,
            build_cmds,
        )
        .await;

        info!("Host setup Successful");
    }

    // run russula
    {
        let mut server_russula = coordination_utils::ServerNetbenchRussula::new(
            &ssm_client,
            &infra,
            server_ids.clone(),
            &client.ip,
        )
        .await;
        let mut client_russula = coordination_utils::ClientNetbenchRussula::new(
            &ssm_client,
            &infra,
            client_ids.clone(),
            &server.ip,
        )
        .await;

        // run client/server
        server_russula.wait_workers_running(&ssm_client).await;
        client_russula.wait_done(&ssm_client).await;
        server_russula.wait_done(&ssm_client).await;
    }

    // copy netbench results
    {
        let copy_server_netbench = ssm_utils::server::copy_netbench_data(
            &ssm_client,
            server_ids.clone(),
            &client.ip,
            &unique_id,
        )
        .await;
        let copy_client_netbench = ssm_utils::client::copy_netbench_data(
            &ssm_client,
            client_ids.clone(),
            &server.ip,
            &unique_id,
        )
        .await;
        ssm_utils::common::wait_complete(
            "client_server_netbench_copy_results",
            &ssm_client,
            vec![copy_server_netbench, copy_client_netbench],
        )
        .await;
        info!("client_server netbench copy results!: Successful");
    }

    // Copy results back
    orch_generate_report(&s3_client, &unique_id).await;

    // Cleanup
    infra
        .cleanup(&ec2_client)
        .await
        .map_err(|err| eprintln!("Failed to cleanup resources. {}", err))
        .unwrap();

    Ok(())
}
