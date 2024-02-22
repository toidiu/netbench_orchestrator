// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::{OrchError, OrchResult},
    STATE,
};
use aws_sdk_ec2::types::Placement as AwsPlacement;
use clap::{Args, Parser};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Command,
};
use tracing::debug;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Path to cdk parameter file
    #[arg(long, default_value = "output.json")]
    cdk_config_file: PathBuf,

    /// Path to the scenario file
    #[arg(long)]
    netbench_scenario_file: PathBuf,

    // An infrastructure overlay for the hosts specified in the
    // netbench scenario file
    #[command(flatten)]
    infra: InfraScenario,
}

impl Cli {
    pub async fn check_requirements(
        self,
        aws_config: &aws_types::SdkConfig,
    ) -> OrchResult<OrchestratorConfig> {
        let (scenario, netbench_scenario_filename) =
            NetbenchScenario::from_file(&self.netbench_scenario_file)?;
        let cdk_config = CdkConfig::from_file(&self.cdk_config_file)?;
        debug!("{:?}", cdk_config);
        let mut client_config = Vec::with_capacity(scenario.clients.len());
        for _az in &self.infra.client_az {
            client_config.push(HostConfig::new());
        }

        let mut server_config = Vec::with_capacity(scenario.servers.len());
        for _az in &self.infra.server_az {
            server_config.push(HostConfig::new());
        }

        let config = OrchestratorConfig {
            netbench_scenario_filename,
            netbench_scenario_filepath: self.netbench_scenario_file.clone(),
            client_config,
            server_config,
            cdk_config,
            placement: self.infra.placement,
        };

        // export PATH="/home/toidiu/projects/s2n-quic/netbench/target/release/:$PATH"
        Command::new("s2n-netbench")
            .output()
            .map_err(|_err| OrchError::Init {
                dbg: "Missing `s2n-netbench` cli. Please the Getting started section in the Readme"
                    .to_string(),
            })?;

        Command::new("aws")
            .output()
            .map_err(|_err| OrchError::Init {
                dbg: "Missing `aws` cli.".to_string(),
            })?;

        // report folder
        std::fs::create_dir_all(STATE.workspace_dir).map_err(|_err| OrchError::Init {
            dbg: "Failed to create local workspace".to_string(),
        })?;

        let iam_client = aws_sdk_iam::Client::new(aws_config);
        iam_client
            .list_roles()
            .send()
            .await
            .map_err(|_err| OrchError::Init {
                dbg: "Missing AWS credentials.".to_string(),
            })?;

        Ok(config)
    }
}

#[derive(Clone, Debug)]
pub struct OrchestratorConfig {
    // netbench
    pub netbench_scenario_filename: String,
    pub netbench_scenario_filepath: PathBuf,
    // cdk
    pub cdk_config: CdkConfig,
    // infra
    pub client_config: Vec<HostConfig>,
    pub server_config: Vec<HostConfig>,
    placement: PlacementGroup,
}

impl OrchestratorConfig {
    pub fn netbench_scenario_file_stem(&self) -> &str {
        self.netbench_scenario_filepath
            .as_path()
            .file_stem()
            .expect("expect scenario file")
            .to_str()
            .unwrap()
    }

    pub fn to_ec2_placement(&self) -> AwsPlacement {
        let mut placement = AwsPlacement::builder();

        // set placement group
        placement = match self.placement {
            PlacementGroup::Cluster => placement.group_name(STATE.placement_group_cluster),
            PlacementGroup::Partition => placement.group_name(STATE.placement_group_partition),
        };

        // set AZ
        // let az = match endpoint_type {
        //     EndpointType::Server => &self.server_az,
        //     EndpointType::Client => &self.client_az,
        // };
        // if let Some(az) = az {
        //     placement = placement.availability_zone(az);
        // }

        placement.build()
    }
}

#[derive(Clone, Debug)]
pub struct HostConfig {
    region: String,
    az: String,
    instance_type: String,
}

impl HostConfig {
    fn new() -> Self {
        HostConfig {
            region: STATE.region.to_owned(),
            az: "us-west-2a".to_owned(),
            instance_type: STATE.instance_type.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Default, Args)]
pub struct InfraScenario {
    /// Placement strategy for the netbench hosts
    #[arg(long, default_value = "cluster")]
    placement: PlacementGroup,

    #[arg(long)]
    client_az: Vec<String>,

    #[arg(long)]
    server_az: Vec<String>,
    // instance_type: String
    // region
    // ssh_key_name
}

// https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/placement-groups.html?icmpid=docs_ec2_console
#[derive(Clone, Debug, Default, clap::ValueEnum)]
pub enum PlacementGroup {
    #[default]
    // Packs instances close together inside an Availability Zone. This
    // strategy enables workloads to achieve the low-latency network
    // performance necessary for tightly-coupled node-to-node communication
    // that is typical of high-performance computing (HPC) applications.
    Cluster,

    // Spreads your instances across logical partitions such that groups of
    // instances in one partition do not share the underlying hardware with
    // groups of instances in different partitions. This strategy is
    // typically used by large distributed and replicated workloads, such as
    // Hadoop, Cassandra, and Kafka.
    Partition,
    // TODO support spread
    // // Strictly places a small group of instances across distinct underlying
    // // hardware to reduce correlated failures.
    // Spread,
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NetbenchScenario {
    pub clients: Vec<Value>,
    pub servers: Vec<Value>,
}

impl NetbenchScenario {
    fn from_file(netbench_scenario_file: &PathBuf) -> OrchResult<(Self, String)> {
        let path = Path::new(&netbench_scenario_file);
        let name = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or(OrchError::Init {
                dbg: "Scenario file not specified".to_string(),
            })?
            .to_string();
        let netbench_scenario_file = File::open(path).map_err(|_err| OrchError::Init {
            dbg: format!("Scenario file not found: {:?}", path),
        })?;
        let scenario: NetbenchScenario = serde_json::from_reader(netbench_scenario_file).unwrap();
        Ok((scenario, name))
    }
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CdkConfig {
    resources: Resources,
}

impl CdkConfig {
    pub fn netbench_runner_s3_bucket(&self) -> &String {
        &self.resources.netbench_runner_s3_bucket
    }

    pub fn netbench_cloudfront_distibution(&self) -> &String {
        &self.resources.netbench_cloudfront_distibution
    }

    pub fn netbench_runner_log_group(&self) -> &String {
        &self.resources.netbench_runner_log_group
    }

    pub fn netbench_runner_instance_profile(&self) -> &String {
        &self.resources.netbench_runner_instance_profile
    }

    pub fn netbench_runner_vpc_subnet_tag_key(&self) -> String {
        format!("tag:{}", self.resources.netbench_runner_vpc_subnet_tag.key)
    }

    pub fn netbench_runner_vpc_subnet_tag_value(&self) -> &String {
        &self.resources.netbench_runner_vpc_subnet_tag.value
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct Resources {
    netbench_runner_log_group: String,
    netbench_runner_s3_bucket: String,
    netbench_cloudfront_distibution: String,
    netbench_runner_instance_profile: String,
    #[serde(rename(deserialize = "NetbenchRunnerVPCSubnetTag"))]
    netbench_runner_vpc_subnet_tag: SubnetTag,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SubnetTag {
    pub key: String,
    pub value: String,
}

impl CdkConfig {
    fn from_file(cdk_config_file: &PathBuf) -> OrchResult<Self> {
        let path = Path::new(&cdk_config_file);
        let cdk_config_file = File::open(path).map_err(|_err| OrchError::Init {
            dbg: format!("Scenario file not found: {:?}", path),
        })?;
        let config: CdkConfig = serde_json::from_reader(cdk_config_file).unwrap();
        Ok(config)
    }
}
