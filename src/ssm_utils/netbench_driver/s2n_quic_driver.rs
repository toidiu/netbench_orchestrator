// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::NetbenchDriver;
use crate::{Scenario, STATE};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::debug;

pub fn quic_server_driver(unique_id: &str, scenario: &Scenario) -> NetbenchDriver {
    let proj_name = "s2n-netbench".to_string();
    let driver = NetbenchDriver {
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
                scenario.name,
                // to
                STATE.host_bin_path(),
                scenario.name
            ),
        ],
        proj_name,
        local_path_to_proj: None,
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

pub fn quic_client_driver(unique_id: &str, scenario: &Scenario) -> NetbenchDriver {
    let proj_name = "s2n-netbench".to_string();
    let driver = NetbenchDriver {
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
                scenario.name,
                // to
                STATE.host_bin_path(),
                scenario.name
            ),
        ],
        proj_name,
        local_path_to_proj: None,
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

// This local command runs twice; once for server and once for client.
// For this reason `aws sync` is preferred over `aws cp` since sync avoids
// object copy if the same copy already exists.
fn local_upload_source_to_s3(local_path_to_proj: &PathBuf, proj_name: &str, unique_id: &str) {
    let mut local_to_s3_cmd = Command::new("aws");
    local_to_s3_cmd.args(["s3", "sync"]).stdout(Stdio::null());
    local_to_s3_cmd
        .arg(format!(
            "{}/{}",
            local_path_to_proj.to_str().unwrap(),
            proj_name
        ))
        .arg(format!("{}/{}/", STATE.s3_path(unique_id), proj_name));
    local_to_s3_cmd.args(["--exclude", "target/*", "--exclude", ".git/*"]);
    debug!("{:?}", local_to_s3_cmd);
    let status = local_to_s3_cmd.status().unwrap();
    assert!(status.success(), "aws sync command failed");
}
