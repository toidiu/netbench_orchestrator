// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use aws_types::region::Region;
use clap::Parser;
use error::{OrchError, OrchResult};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};
use tracing_subscriber::EnvFilter;

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
// - instance::poll_state should take multiple instance_ids
//
// - install netbench drivers from crates.io
// - save hash of private source
//   - get private src exec from s3
// - cleanup dashboard
// - enum for orch steps
//   - add timing data
//
// # Expanding Russula/Cli
//
// # Optimization
// - tar.gz private source
// - enum for driver build type (git, source, crates.io)
//
// - use release build instead of debug
// - experiment with uploading and downloading netbench exec

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the scenario file
    #[arg(long, default_value = "scripts/request_response.json")]
    scenario_file: PathBuf,
}

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

    let args = Args::parse();

    let region = Region::new(STATE.region);
    let aws_config = aws_config::from_env().region(region).load().await;
    let scenario = check_requirements(&args, &aws_config).await?;

    orchestrator::run(unique_id, args, scenario, &aws_config).await
}

async fn check_requirements(
    args: &Args,
    aws_config: &aws_types::SdkConfig,
) -> OrchResult<Scenario> {
    let path = Path::new(&args.scenario_file);
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

    let ctx = Scenario {
        name,
        path: args.scenario_file.clone(),
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

// FIXME get from netbench project
#[derive(Clone, Debug, Default, Deserialize)]
struct NetbenchScenario {
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

#[derive(Clone, Debug)]
pub struct Scenario {
    name: String,
    path: PathBuf,
    clients: usize,
    servers: usize,
}

impl Scenario {
    pub fn file_stem(&self) -> &str {
        self.path
            .as_path()
            .file_stem()
            .expect("expect scenario file")
            .to_str()
            .unwrap()
    }
}
