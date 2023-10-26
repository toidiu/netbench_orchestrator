use crate::ec2_utils::EndpointType;
use core::time::Duration;

pub const STATE: State = State {
    version: "v1.0.17",

    // git
    repo: "https://github.com/aws/s2n-quic.git",
    branch: "ak-netbench-sync",

    // aws
    log_bucket: "netbenchrunnerlogs",
    cf_url: "http://d2jusruq1ilhjs.cloudfront.net", // TODO use in code
    cloud_watch_group: "netbench_runner_logs",
    region: "us-west-1",
    vpc_region: "us-east-1",
    instance_type: "c5.4xlarge",
    // Used to give permissions to the ec2 instance. Part of the `NetbenchRunnerRole`
    instance_profile: "NetbenchRunnerInstanceProfile",
    // Used to find subnets with the following tag/value pair
    subnet_tag_value: (
        "tag:aws-cdk:subnet-name",
        "public-subnet-for-runners-in-us-east-1",
    ),
    // create/import a key pair to the account
    ssh_key_name: "apoorvko_m1",

    // orchestrator config
    host_count: HostCount {
        clients: 3,
        servers: 2,
    },
    workspace_dir: "./target/netbench",
    shutdown_time_sec: Duration::from_secs(60),
};

pub struct State {
    pub version: &'static str,
    // git
    pub repo: &'static str,
    pub branch: &'static str,

    // aws
    pub log_bucket: &'static str,
    pub cf_url: &'static str,
    pub cloud_watch_group: &'static str,
    pub region: &'static str,
    // TODO we shouldnt need two different regions. create infra in the single region
    pub vpc_region: &'static str,
    pub instance_type: &'static str,
    pub instance_profile: &'static str,
    pub subnet_tag_value: (&'static str, &'static str),
    pub ssh_key_name: &'static str,

    // orchestrator config
    pub host_count: HostCount,
    pub workspace_dir: &'static str,
    pub shutdown_time_sec: Duration,
}

#[derive(Clone)]
pub struct HostCount {
    pub clients: u16,
    pub servers: u16,
}

impl State {
    pub fn cf_url_with_id(&self, id: &str) -> String {
        format!("{}/{}", self.cf_url, id)
    }

    // Create a security group with the following name prefix. Use with `sg_name_with_id`
    // security_group_name_prefix: "netbench_runner",
    pub fn sg_name_with_id(&self, unique_id: &str) -> String {
        format!("netbench_{}", unique_id)
    }

    pub fn instance_name(&self, unique_id: &str, endpoint_type: EndpointType) -> String {
        format!("{}_{}", endpoint_type.as_str().to_lowercase(), unique_id)
    }
}
