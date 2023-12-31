// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::ec2_utils::EndpointType;
use core::time::Duration;

pub const STATE: State = State {
    version: "v2.0.0",

    // netbench
    netbench_repo: "https://github.com/aws/s2n-quic.git",
    netbench_branch: "ak-netbench-sync",

    // orchestrator
    host_count: HostCount {
        clients: 1,
        servers: 1,
    },
    workspace_dir: "./target/netbench",
    shutdown_min: 120, // 1 hour
    poll_cmds_duration: Duration::from_secs(10),

    // russula
    russula_repo: "https://github.com/toidiu/netbench_orchestrator.git",
    russula_branch: "ak-main",
    russula_port: 9000,

    // aws
    s3_log_bucket: "netbenchrunnerlogs",
    // TODO contains request_response.json but that should just come from the orchestrator
    s3_resource_folder: "TS",
    cloudfront_url: "http://d2jusruq1ilhjs.cloudfront.net",
    cloud_watch_group: "netbench_runner_logs",
    // TODO remove `vpc_region` and configure vpc/subnet in same `region`
    region: "us-west-1",
    vpc_region: "us-east-1",
    instance_type: "c5.4xlarge",
    // Used to give permissions to the ec2 instance. Part of the IAM Role `NetbenchRunnerRole`
    instance_profile: "NetbenchRunnerInstanceProfile",
    // Used to find subnets with the following tag/value pair
    subnet_tag_value: (
        "tag:aws-cdk:subnet-name",
        "public-subnet-for-runners-in-us-east-1",
    ),
    // create/import a key pair to the account
    ssh_key_name: "apoorvko_m1",
};

pub struct State {
    pub version: &'static str,
    // netbench
    pub netbench_repo: &'static str,
    pub netbench_branch: &'static str,

    // orchestrator
    pub host_count: HostCount,
    pub workspace_dir: &'static str,
    pub shutdown_min: u16,
    pub poll_cmds_duration: Duration,

    // russula
    pub russula_repo: &'static str,
    pub russula_branch: &'static str,
    pub russula_port: i32,

    // aws
    pub s3_log_bucket: &'static str,
    pub s3_resource_folder: &'static str,
    pub cloudfront_url: &'static str,
    pub cloud_watch_group: &'static str,
    pub region: &'static str,
    // TODO we shouldnt need two different regions. create infra in the single region
    pub vpc_region: &'static str,
    pub instance_type: &'static str,
    pub instance_profile: &'static str,
    pub subnet_tag_value: (&'static str, &'static str),
    pub ssh_key_name: &'static str,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostCount {
    pub clients: u16,
    pub servers: u16,
}

impl State {
    pub fn cf_url(&self, unique_id: &str) -> String {
        format!("{}/{}", self.cloudfront_url, unique_id)
    }

    pub fn s3_path(&self, unique_id: &str) -> String {
        format!("s3://{}/{}", self.s3_log_bucket, unique_id)
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
