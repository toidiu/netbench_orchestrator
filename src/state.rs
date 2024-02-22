// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::ec2_utils::EndpointType;
use crate::OrchestratorConfig;
use core::time::Duration;

pub const STATE: State = State {
    version: "v2.2.1",

    // TODO remove `vpc_region` and configure vpc/subnet in same `region`
    region: "us-west-2",
    vpc_region: "us-west-2",
    instance_type: "c5.4xlarge",
    // TODO get from scenario --------------

    // netbench
    netbench_repo: "https://github.com/aws/s2n-netbench.git",
    netbench_branch: "main",
    netbench_port: 4433,

    // orchestrator
    host_home_path: "/home/ec2-user",
    workspace_dir: "./target/netbench",
    shutdown_min: 120, // 1 hour
    poll_delay_ssm: Duration::from_secs(10),

    // russula
    russula_repo: "https://github.com/toidiu/netbench_orchestrator.git",
    russula_branch: "ak-main",
    russula_port: 9000,
    poll_delay_russula: Duration::from_secs(5),

    // aws
    placement_group_cluster: "NetbenchInfraPrimaryProd-ClusterEB0386A7-R5EWN2RCJC5L",
    placement_group_partition: "NetbenchInfraPrimaryProd-Partition9E68ED67-LC9VKTZBNJJ8",
    // json
    s3_private_log_bucket: "netbenchrunner-private-source-prod",
    // create/import a key pair to the account
    ssh_key_name: None,
    // ssh_key_name: Some("apoorvko_m1"),
};

pub struct State {
    pub version: &'static str,

    // TODO get from scenario --------------
    // pub host_count: HostCount,
    pub region: &'static str,
    // TODO we shouldnt need two different regions. create infra in the single region
    pub vpc_region: &'static str,
    pub instance_type: &'static str,
    // TODO get from scenario --------------

    // netbench
    pub netbench_repo: &'static str,
    pub netbench_branch: &'static str,
    pub netbench_port: u16,

    // orchestrator
    pub host_home_path: &'static str,
    pub workspace_dir: &'static str,
    pub shutdown_min: u16,
    pub poll_delay_ssm: Duration,

    // russula
    pub russula_repo: &'static str,
    pub russula_branch: &'static str,
    pub russula_port: u16,
    pub poll_delay_russula: Duration,

    // aws
    pub placement_group_cluster: &'static str,
    pub placement_group_partition: &'static str,
    pub s3_private_log_bucket: &'static str,
    pub ssh_key_name: Option<&'static str>,
}

impl State {
    pub fn cf_url(&self, unique_id: &str, config: &OrchestratorConfig) -> String {
        format!(
            "{}/{}",
            config.cdk_config.netbench_cloudfront_distibution(),
            unique_id
        )
    }

    pub fn s3_path(&self, unique_id: &str, config: &OrchestratorConfig) -> String {
        format!(
            "s3://{}/{}",
            config.cdk_config.netbench_runner_s3_bucket(),
            unique_id
        )
    }

    pub fn s3_private_path(&self, unique_id: &str) -> String {
        format!("s3://{}/{}", self.s3_private_log_bucket, unique_id)
    }

    pub fn host_bin_path(&self) -> String {
        format!("{}/bin", self.host_home_path)
    }

    pub fn cargo_path(&self) -> String {
        format!("{}/bin/cargo", self.host_home_path)
    }

    // Create a security group with the following name prefix. Use with `sg_name_with_id`
    // security_group_name_prefix: "netbench_runner",
    pub fn security_group_name(&self, unique_id: &str) -> String {
        format!("netbench_{}", unique_id)
    }

    pub fn instance_name(&self, unique_id: &str, endpoint_type: EndpointType) -> String {
        format!("{}_{}", endpoint_type.as_str().to_lowercase(), unique_id)
    }
}
