// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};
use crate::OrchestratorScenario;

pub fn native_tls_server_driver(
    unique_id: &str,
    scenario: &OrchestratorScenario,
) -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-native-tls".to_string(),
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        // repo_name: crate_name.clone(),
        driver_name: "s2n-netbench-driver-server-native-tls".to_string(),
        version: "*".to_string(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn native_tls_client_driver(
    unique_id: &str,
    scenario: &OrchestratorScenario,
) -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-native-tls".to_string(),
        driver_name: "s2n-netbench-driver-client-native-tls".to_string(),
        version: "*".to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
        unique_id: unique_id.to_string(),
        // repo_name: crate_name.clone(),
    };
    NetbenchDriverType::CratesIo(source)
}
