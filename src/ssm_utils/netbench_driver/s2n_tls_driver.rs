// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{CrateIoSource, NetbenchDriverType};
use crate::OrchestratorScenario;

pub fn s2n_tls_server_driver(
    unique_id: &str,
    scenario: &OrchestratorScenario,
) -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-tls".to_string(),
        driver_name: "s2n-netbench-driver-server-s2n-tls".to_string(),
        version: "*".to_string(),
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
    };
    NetbenchDriverType::CratesIo(source)
}

pub fn s2n_tls_client_driver(
    unique_id: &str,
    scenario: &OrchestratorScenario,
) -> NetbenchDriverType {
    let source = CrateIoSource {
        krate: "s2n-netbench-driver-s2n-tls".to_string(),
        driver_name: "s2n-netbench-driver-client-s2n-tls".to_string(),
        version: "*".to_string(),
        unique_id: unique_id.to_string(),
        netbench_scenario_filename: scenario.netbench_scenario_filename.clone(),
    };
    NetbenchDriverType::CratesIo(source)
}
