// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]
use aws_types::region::Region;
use clap::Parser;
use error::OrchResult;
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
    let config = cli.check_requirements(&aws_config).await?;

    orchestrator::run(unique_id, &config, &aws_config, false).await
}
