// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::NetbenchDriver;
use crate::{ssm_utils::netbench_driver::local_upload_source_to_s3, OrchestratorScenario, STATE};

pub fn dc_quic_server_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriver {
    let proj_name = "SaltyLib-Rust".to_string();
    let driver = NetbenchDriver {
        driver_name: "s2n-netbench-driver-server-s2n-quic-dc".to_string(),
        ssm_build_cmd: vec![
            // copy s3 to host: `aws s3 sync s3://netbenchrunnerlogs-source/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_private_path(unique_id),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!(
                "env RUSTFLAGS='--cfg s2n_quic_unstable' {}/cargo build",
                STATE.host_bin_path()
            ),
            // copy executables to bin directory
            format!(
                "find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
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
        proj_name: proj_name.clone(),
        local_path_to_proj: Some("/Users/apoorvko/projects/ws_SaltyLib/src".into()),
    };

    // TODO move this one layer up so its common
    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

pub fn dc_quic_client_driver(unique_id: &str, scenario: &OrchestratorScenario) -> NetbenchDriver {
    let proj_name = "SaltyLib-Rust".to_string();
    let driver = NetbenchDriver {
        driver_name: "s2n-netbench-driver-client-s2n-quic-dc".to_string(),
        ssm_build_cmd: vec![
            // copy s3 to host
            // `aws s3 sync s3://netbenchrunnerlogs/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_private_path(unique_id),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!(
                "env RUSTFLAGS='--cfg s2n_quic_unstable' {}/cargo build",
                STATE.host_bin_path()
            ),
            // copy executables to bin directory
            format!(
                "find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
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
        proj_name: proj_name.clone(),
        local_path_to_proj: Some("/Users/apoorvko/projects/ws_SaltyLib/src".into()),
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}
