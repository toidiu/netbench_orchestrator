// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use aws_types::region::Region;
use clap::Parser;
use error::{OrchError, OrchResult};
use std::{fs::File, path::Path, process::Command};
use tracing_subscriber::EnvFilter;

mod cli;
mod coordination_utils;
mod dashboard;
mod duration;
mod ec2_utils;
mod error;
mod orchestrator;
mod report;
mod russula;
mod s3_utils;
mod ssm_utils;
mod state;

use cli::*;
use dashboard::*;
use ec2_utils::*;
use s3_utils::*;
use ssm_utils::*;
use state::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    let unique_id = format!(
        "{}-{}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
        STATE.version
    );

    // tracing_subscriber::fmt::init();
    let file_appender =
        tracing_appender::rolling::daily("./target", format!("russula_{}.log", unique_id));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(non_blocking)
        .init();

    let cli = Cli::parse();

    let region = Region::new(STATE.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    let scenario = check_requirements(&cli, &aws_config).await?;

    orchestrator::run(unique_id, cli, scenario, &aws_config).await
}

async fn check_requirements(
    cli: &Cli,
    aws_config: &aws_types::SdkConfig,
) -> OrchResult<OrchestratorScenario> {
    let path = Path::new(&cli.scenario_file);
    let name = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or(OrchError::Init {
            dbg: "Scenario file not specified".to_string(),
        })?
        .to_string();
    let scenario_file = File::open(path).map_err(|_err| OrchError::Init {
        dbg: format!("Scenario file not found: {:?}", path),
    })?;
    let scenario: NetbenchScenario = serde_json::from_reader(scenario_file).unwrap();

    let ctx = OrchestratorScenario {
        netbench_scenario_filename: name,
        netbench_scenario_filepath: cli.scenario_file.clone(),
        clients: scenario.clients.len(),
        servers: scenario.servers.len(),
    };

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

    let iam_client = aws_sdk_iam::Client::new(aws_config);
    iam_client
        .list_roles()
        .send()
        .await
        .map_err(|_err| OrchError::Init {
            dbg: "Missing AWS credentials.".to_string(),
        })?;

    Ok(ctx)
}
