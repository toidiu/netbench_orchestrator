// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use aws_types::region::Region;
use clap::Parser;
use error::{OrchError, OrchResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};

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
    #[arg(long, default_value = "request_response.json")]
    scenario_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> OrchResult<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let region = Region::new(STATE.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    check_requirements(&args, &aws_config).await?;

    orchestrator::run(args, &aws_config).await
}

async fn check_requirements(args: &Args, aws_config: &aws_types::SdkConfig) -> OrchResult<()> {
    if !Path::new(&args.scenario_file).exists() {
        return Err(OrchError::Init {
            dbg: "Scenario file doesn't exist".to_string(),
        });
    }
    let path = Path::new(&args.scenario_file);
    path.file_name()
        .map(|f| f.to_str())
        .ok_or(OrchError::Init {
            dbg: "Scenario file not specified".to_string(),
        })?;
    let scenario_file = File::open(path).map_err(|_err| OrchError::Init {
        dbg: "Scenario file not specified".to_string(),
    })?;
    let Scenario { clients, servers } = serde_json::from_reader(scenario_file).unwrap();
    println!("{} {}", clients.len(), servers.len());

    // let _scenario_name = args
    //     .scenario_file
    //     .as_path()
    //     .file_name()
    //     .map(|f| f.to_str())
    //     .ok_or(OrchError::Init {
    //         dbg: "Scenario file not specified".to_string(),
    //     })?;

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

// FIXME get from netbench project
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Scenario {
    // pub id: Id,
    pub clients: Vec<Value>,
    pub servers: Vec<Value>,
    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub routers: Vec<Arc<Router>>,
    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub traces: Arc<Vec<String>>,
    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub certificates: Vec<Arc<Certificate>>,
}
