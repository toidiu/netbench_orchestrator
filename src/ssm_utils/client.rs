// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{state::STATE, NetbenchDriver, OrchestratorScenario};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::net::{IpAddr, SocketAddr};
use tracing::debug;

pub async fn upload_netbench_data(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    unique_id: &str,
    scenario: &OrchestratorScenario,
    driver: &NetbenchDriver,
) -> SendCommandOutput {
    let driver_name = driver
        .driver_name
        .trim_start_matches("s2n-netbench-driver-")
        .trim_start_matches("netbench-driver-")
        .trim_end_matches(".json");

    send_command(
        vec![Step::RunRussula],
        Step::UploadNetbenchRawData,
        "client",
        "upload_netbench_raw_data",
        ssm_client,
        instance_ids,
        vec![
            "cd netbench_orchestrator",
            format!(
                "aws s3 cp client* {}/results/{}/{driver_name}/",
                STATE.s3_path(unique_id),
                scenario.netbench_scenario_file_stem()
            )
            .as_str(),
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    )
    .await
    .expect("Timed out")
}

pub async fn run_russula_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    server_ips: &Vec<IpAddr>,
    driver: &NetbenchDriver,
    scenario: &OrchestratorScenario,
) -> SendCommandOutput {
    let netbench_server_addr = server_ips
        .iter()
        .map(|ip| SocketAddr::new(*ip, STATE.netbench_port).to_string())
        .reduce(|mut accum, item| {
            accum.push(' ');
            accum.push_str(&item);
            accum
        })
        .unwrap();

    let netbench_cmd =
        format!("env RUST_LOG=debug ./target/debug/russula_cli netbench-client-worker --russula-port {} --driver {} --scenario {} --netbench-servers {netbench_server_addr}",
            STATE.russula_port, driver.driver_name, scenario.netbench_scenario_filename);
    debug!("{}", netbench_cmd);

    send_command(
        vec![Step::BuildDriver("".to_string()), Step::BuildRussula],
        Step::RunRussula,
        "client",
        "run_client_russula",
        ssm_client,
        instance_ids,
        vec!["cd netbench_orchestrator", netbench_cmd.as_str()]
            .into_iter()
            .map(String::from)
            .collect(),
    )
    .await
    .expect("Timed out")
}
