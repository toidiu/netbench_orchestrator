// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{GithubSource, NetbenchDriverType};
use crate::OrchestratorScenario;

pub fn tcp_server_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        driver_name: "s2n-netbench-driver-server-tcp".to_string(),
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::GithubRustProj(source)
}

pub fn tcp_client_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let repo_name = "s2n-netbench".to_string();
    let source = GithubSource {
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        driver_name: "s2n-netbench-driver-client-tcp".to_string(),
        repo_name: repo_name.clone(),
    };

    NetbenchDriverType::GithubRustProj(source)
}
