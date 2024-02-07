// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use std::path::PathBuf;

mod native_tls_driver;
mod s2n_quic_dc_driver;
mod s2n_quic_driver;
mod s2n_tls_driver;
mod tcp_driver;

pub use native_tls_driver::*;
pub use s2n_quic_dc_driver::*;
pub use s2n_quic_driver::*;
pub use s2n_tls_driver::*;
pub use tcp_driver::*;

pub enum NetbenchDriverType {
    GithubRustProj(GithubSource),
    CratesIo(CrateIoSource),
    Local(LocalSource),
}

pub struct GithubSource {
    pub driver_name: String,
    pub repo_name: String,

    unique_id: String,
    // TODO remove by uploading scenario file separately
    netbench_scenario_filename: String,
}

pub struct LocalSource {
    pub driver_name: String,
    pub ssm_build_cmd: Vec<String>,
    pub proj_name: String,
    // Used to copy local driver source to hosts
    //
    // upload to s3 locally and download form s3 in ssm_build_cmd
    local_path_to_proj: PathBuf,
    unique_id: String,
    // TODO remove by uploading scenario file separately
    netbench_scenario_filename: String,
}

pub struct CrateIoSource {
    pub krate: String,
    pub driver_name: String,
    version: String,
    // pub repo_name: String,
    unique_id: String,
    // TODO remove by uploading scenario file separately
    netbench_scenario_filename: String,
}

impl NetbenchDriverType {
    pub fn driver_name(&self) -> &String {
        match self {
            NetbenchDriverType::GithubRustProj(source) => &source.driver_name,
            NetbenchDriverType::Local(source) => &source.driver_name,
            NetbenchDriverType::CratesIo(source) => &source.driver_name,
        }
    }

    // Base project name
    // pub fn proj_name(&self) -> &String {
    //     match self {
    //         NetbenchDriverType::GithubRustProj(source) => &source.repo_name,
    //         NetbenchDriverType::Local(source) => &source.proj_name,
    //         NetbenchDriverType::CratesIo(source) => &source.driver_name,
    //     }
    // }

    pub fn ssm_build_cmd(&self) -> Vec<String> {
        match self {
            NetbenchDriverType::GithubRustProj(source) => source.ssm_build_rust_proj(),
            NetbenchDriverType::Local(source) => source.ssm_build_cmd.clone(),
            NetbenchDriverType::CratesIo(source) => source.ssm_build_crates_io_proj(),
        }
    }

    fn unique_id(&self) -> &str {
        match self {
            NetbenchDriverType::GithubRustProj(source) => &source.unique_id,
            NetbenchDriverType::Local(source) => &source.unique_id,
            NetbenchDriverType::CratesIo(source) => &source.unique_id,
        }
    }

    fn netbench_scenario_filename(&self) -> &str {
        match self {
            NetbenchDriverType::GithubRustProj(source) => &source.netbench_scenario_filename,
            NetbenchDriverType::Local(source) => &source.netbench_scenario_filename,
            NetbenchDriverType::CratesIo(source) => &source.netbench_scenario_filename,
        }
    }
}

impl GithubSource {
    pub fn ssm_build_rust_proj(&self) -> Vec<String> {
        let unique_id = &self.unique_id;
        vec![
            format!(
                "git clone --branch {} {}",
                STATE.netbench_branch, STATE.netbench_repo
            ),
            format!("cd {}", self.repo_name),
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
                self.netbench_scenario_filename,
                // to
                STATE.host_bin_path(),
                self.netbench_scenario_filename
            ),
        ]
    }
}

impl CrateIoSource {
    pub fn ssm_build_crates_io_proj(&self) -> Vec<String> {
        let unique_id = &self.unique_id;
        vec![
            format!(
                "runuser -u ec2-user -- {}/cargo install s2n-netbench-collector",
                STATE.host_bin_path(),
            ),
            format!(
                "ln -s /home/ec2-user/.cargo/bin/s2n-netbench-collector {}/s2n-netbench-collector",
                STATE.host_bin_path(),
            ),
            format!(
                // "runuser -u ec2-user -- ./.cargo/bin/rustup update".to_string(),
                "runuser -u ec2-user -- {}/cargo install {}",
                STATE.host_bin_path(),
                self.krate,
                // self.version
            ),
            // link this from /bin folder
            format!(
                "ln -s /home/ec2-user/.cargo/bin/{} {}/{}",
                self.driver_name,
                STATE.host_bin_path(),
                self.driver_name,
            ),
            // copy scenario file to host
            format!(
                "aws s3 cp s3://{}/{unique_id}/{} {}/{}",
                // from
                STATE.s3_log_bucket,
                self.netbench_scenario_filename,
                // to
                STATE.host_bin_path(),
                self.netbench_scenario_filename
            ),
        ]
    }
}
