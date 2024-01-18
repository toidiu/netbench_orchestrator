// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

mod s2n_quic_dc_driver;
mod s2n_quic_driver;

pub use s2n_quic_dc_driver::*;
pub use s2n_quic_driver::*;

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
