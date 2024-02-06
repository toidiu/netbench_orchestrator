// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{GithubSource, NetbenchDriverType};
use crate::{OrchestratorScenario, STATE};

pub fn quic_server_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        driver_name: "s2n-netbench-driver-server-s2n-quic".to_string(),
        ssm_build_cmd: vec![
            format!(
                "git clone --branch {} {}",
                STATE.netbench_branch, STATE.netbench_repo
            ),
            format!("cd {}", proj_name),
            format!("{}/cargo build --release", STATE.host_bin_path()),
            // copy netbench executables to ~/bin folder
            format!(
                "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                STATE.host_bin_path()
            ),
            // copy scenario file to host
            format!(
                "aws s3 cp s3://{}/{unique_id}/{} {}/{}",
                // from
                STATE.s3_log_bucket,
                scenario.netbench_scenario_filename,
                // to
                STATE.host_bin_path(),
                scenario.netbench_scenario_filename
            ),
        ],
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::Github(source)
}

pub fn quic_client_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriverType {
    let proj_name = "s2n-netbench".to_string();

    let source = GithubSource {
        driver_name: "s2n-netbench-driver-client-s2n-quic".to_string(),
        ssm_build_cmd: vec![
            format!(
                "git clone --branch {} {}",
                STATE.netbench_branch, STATE.netbench_repo
            ),
            format!("cd {}", proj_name),
            format!("{}/cargo build --release", STATE.host_bin_path()),
            // copy netbench executables to ~/bin folder
            format!(
                "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                STATE.host_bin_path()
            ),
            // copy scenario file to host
            format!(
                "aws s3 cp s3://{}/{unique_id}/{} {}/{}",
                // from
                STATE.s3_log_bucket,
                scenario.netbench_scenario_filename,
                // to
                STATE.host_bin_path(),
                scenario.netbench_scenario_filename
            ),
        ],
        repo_name: proj_name.clone(),
    };
    NetbenchDriverType::Github(source)
}
