// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::debug;

mod s2n_quic_dc_driver;
mod s2n_quic_driver;
mod tcp_driver;

pub use s2n_quic_dc_driver::*;
pub use s2n_quic_driver::*;
pub use tcp_driver::*;

pub struct NetbenchDriver {
    pub driver_name: String,
    pub ssm_build_cmd: Vec<String>,
    // Usually the Github repo name
    pub proj_name: String,
    // used to copy local driver source to hosts
    //
    // upload to s3 locally and download form s3 in ssm_build_cmd
    local_path_to_proj: Option<PathBuf>,
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
        .arg(format!(
            "{}/{}/",
            STATE.s3_private_path(unique_id),
            proj_name
        ));
    local_to_s3_cmd.args(["--exclude", "target/*", "--exclude", ".git/*"]);
    debug!("{:?}", local_to_s3_cmd);
    let status = local_to_s3_cmd.status().unwrap();
    assert!(status.success(), "aws sync command failed");
}
