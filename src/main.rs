// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use aws_types::region::Region;
use clap::Parser;
use error::{OrchError, OrchResult};
use std::{path::PathBuf, process::Command};

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

use dashboard::*;
use ec2_utils::*;
use s3_utils::*;
use ssm_utils::*;
use state::*;

// TODO
// - clap app
// - upload request_response.json
// - get STATE config from infra.json and scenario.json
// - save netbench output to different named files instead of server.json/client.json
//
// # Expanding Russula/Cli
// - pass netbench_path to russula_cli
// - pass scenario to russula_cli
// - pass scenario and path from coord -> worker?
// - replace russula_cli russula_port with russula_pair_addr_list
//
// # Optimization
// - use release build instead of debug
// - experiment with uploading and downloading netbench exec

#[derive(Parser, Debug)]
pub struct Args {
    /// Path the scenario file
    #[arg(long)]
    scenario_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let region = Region::new(STATE.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    check_requirements(&aws_config).await?;

    orchestrator::run(args, &aws_config).await
}

async fn check_requirements(aws_config: &aws_types::SdkConfig) -> OrchResult<()> {
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

    Ok(())
}
