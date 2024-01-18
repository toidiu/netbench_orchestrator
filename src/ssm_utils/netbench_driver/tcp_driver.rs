// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{local_upload_source_to_s3, NetbenchDriver};
use crate::STATE;

pub fn tcp_server_driver(unique_id: &str) -> NetbenchDriver {
    let proj_name = "s2n-netbench".to_string();
    let driver = NetbenchDriver {
        driver_name: "s2n-netbench-driver-server-tcp".to_string(),
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
                "aws s3 cp s3://{}/{}/request_response.json {}/request_response.json",
                STATE.s3_log_bucket,
                STATE.s3_resource_folder,
                STATE.host_bin_path()
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

pub fn tcp_client_driver(unique_id: &str) -> NetbenchDriver {
    let proj_name = "s2n-netbench".to_string();
    let driver = NetbenchDriver {
        driver_name: "s2n-netbench-driver-client-tcp".to_string(),
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
                "aws s3 cp s3://{}/{}/request_response.json {}/request_response.json",
                STATE.s3_log_bucket,
                STATE.s3_resource_folder,
                STATE.host_bin_path()
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
