// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{GithubSource, NetbenchDriverType};
use crate::OrchestratorScenario;

pub fn quic_server_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        driver_name: "s2n-netbench-driver-server-s2n-quic".to_string(),
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::GithubRustProj(source)
}

pub fn quic_client_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        driver_name: "s2n-netbench-driver-client-s2n-quic".to_string(),
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::GithubRustProj(source)
}
