// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::debug;

pub struct NetbenchDriver {
    pub driver_name: String,
    pub build_cmd: Vec<String>,
    // Usually the Github repo name
    pub proj_name: String,
    // used to copy local driver source to hosts
    local_path_to_proj: Option<PathBuf>,
}

impl NetbenchDriver {
    pub fn remote_path_to_driver(&self) -> String {
        format!("{}/{}", STATE.host_bin_path(), self.proj_name)
    }
}

pub fn quic_server_driver(unique_id: &str) -> NetbenchDriver {
    let proj_name = "s2n-netbench".to_string();
    let driver = NetbenchDriver {
        driver_name: "netbench-driver-s2n-quic-server".to_string(),
        build_cmd: vec![
            // format!(
            //     "git clone --branch {} {}",
            //     STATE.netbench_branch, STATE.netbench_repo
            // ),
            // format!("cd {}", proj_name),
            // format!("{}/cargo build --release", STATE.host_bin_path()),
            // // copy netbench executables to ~/bin folder
            // format!(
            //     "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
            //     STATE.host_bin_path()
            // ),
            // // copy scenario file to host
            // format!(
            //     "aws s3 cp s3://{}/{}/request_response.json {}/request_response.json",
            //     STATE.s3_log_bucket,
            //     STATE.s3_resource_folder,
            //     STATE.host_bin_path()
            // ),
        ],
        proj_name: proj_name.clone(),
        local_path_to_proj: None,
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

pub fn quic_client_driver(unique_id: &str) -> NetbenchDriver {
    let name = "todo".to_string();
    let driver = NetbenchDriver {
        driver_name: "netbench-driver-s2n-quic-client".to_string(),
        build_cmd: vec![
            format!("cd {}", name),
        ],
        proj_name: name.clone(),
        local_path_to_proj: None,
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

pub fn saltylib_server_driver(unique_id: &str) -> NetbenchDriver {
    let proj_name = "SaltyLib-Rust".to_string();
    let driver = NetbenchDriver {
        driver_name: "netbench-driver-s2n-quic-dc-server".to_string(),
        build_cmd: vec![
            // copy s3 to host
            // `aws s3 sync s3://netbenchrunnerlogs/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_path(unique_id),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!("env RUSTFLAGS='--cfg s2n_quic_unstable' {}/cargo build", STATE.host_bin_path()),
            // copy executables to bin directory
            format!("find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;", STATE.host_bin_path()),
        ],
        proj_name: proj_name.clone(),
        local_path_to_proj: Some("/Users/apoorvko/projects/ws_SaltyLib/src".into()),
    };

    if let Some(local_path_to_proj) = &driver.local_path_to_proj {
        local_upload_source_to_s3(local_path_to_proj, &driver.proj_name, unique_id);
    }

    driver
}

pub fn saltylib_client_driver(unique_id: &str) -> NetbenchDriver {
    let proj_name = "SaltyLib-Rust".to_string();
    let driver = NetbenchDriver {
        driver_name: "netbench-driver-s2n-quic-dc-client".to_string(),
        build_cmd: vec![
            // copy s3 to host
            // `aws s3 sync s3://netbenchrunnerlogs/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_path(unique_id),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!("env RUSTFLAGS='--cfg s2n_quic_unstable' {}/cargo build", STATE.host_bin_path()),
            // copy executables to bin directory
            format!("find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;", STATE.host_bin_path()),
        ],
        proj_name: proj_name.clone(),
        local_path_to_proj: Some("/Users/apoorvko/projects/ws_SaltyLib/src".into()),
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
