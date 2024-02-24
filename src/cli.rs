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
    infra: CliInfraScenario,
}

impl Cli {
    pub async fn check_requirements(
        self,
        aws_config: &aws_types::SdkConfig,
    ) -> OrchResult<OrchestratorConfig> {
        let (scenario, netbench_scenario_filename) =
            CliNetbenchScenario::from_file(&self.netbench_scenario_file)?;
        let cdk_config = CdkConfig::from_file(&self.cdk_config_file)?;

        // AZ
        assert_eq!(
            self.infra.server_az.len(),
            scenario.servers.len(),
            "AZ overlay should match the number of server hosts in the netbench scenario"
        );
        assert_eq!(
            self.infra.client_az.len(),
            scenario.clients.len(),
            "AZ overlay should match the number of client hosts in the netbench scenario"
        );
        // Placement
        assert!(
            self.infra.server_placement.is_empty()
                || self.infra.server_placement.len() == scenario.servers.len(),
            "Placement overlay should be empty or match the number of client hosts in the netbench scenario"
        );
        assert!(
            self.infra.client_placement.is_empty()
                || self.infra.client_placement.len() == scenario.clients.len(),
            "Placement overlay should be empty or match the number of client hosts in the netbench scenario"
        );

        let mut client_config = Vec::with_capacity(self.infra.client_az.len());
        for (i, az) in self.infra.client_az.into_iter().enumerate() {
            let placement = self
                .infra
                .client_placement
                .get(i)
                .unwrap_or(&PlacementGroupConfig::Unspecified);
            client_config.push(HostConfig::new(az, placement.clone()));
        }
        let mut server_config = Vec::with_capacity(self.infra.server_az.len());
        for (i, az) in self.infra.server_az.into_iter().enumerate() {
            let placement = self
                .infra
                .client_placement
                .get(i)
                .unwrap_or(&PlacementGroupConfig::Unspecified);
            server_config.push(HostConfig::new(az, placement.clone()));
        }

        let config = OrchestratorConfig {
            netbench_scenario_filename,
            netbench_scenario_filepath: self.netbench_scenario_file.clone(),
            client_config,
            server_config,
            cdk_config,
        };
        debug!("{:?}", config);

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

#[derive(Clone, Debug, Default, Args)]
struct CliInfraScenario {
    /// Placement strategy for the netbench hosts
    #[arg(long, value_delimiter = ',')]
    client_placement: Vec<PlacementGroupConfig>,

    #[arg(long, value_delimiter = ',')]
    server_placement: Vec<PlacementGroupConfig>,

    #[arg(long, value_delimiter = ',')]
    client_az: Vec<String>,

    #[arg(long, value_delimiter = ',')]
    server_az: Vec<String>,
    // instance_type: String
    // region
    // ssh_key_name
}

// https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/placement-groups.html?icmpid=docs_ec2_console
#[derive(Clone, Debug, Default, clap::ValueEnum)]
enum PlacementGroupConfig {
    #[default]
    Unspecified,

    // Packs instances close together inside an Availability Zone. This
    // strategy enables workloads to achieve the low-latency network
    // performance necessary for tightly-coupled node-to-node communication
    // that is typical of high-performance computing (HPC) applications.
    Cluster,
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
struct CliNetbenchScenario {
    pub clients: Vec<Value>,
    pub servers: Vec<Value>,
}

impl CliNetbenchScenario {
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
        let scenario: CliNetbenchScenario =
            serde_json::from_reader(netbench_scenario_file).unwrap();
        Ok((scenario, name))
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
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CdkConfig {
    resources: CdkResourcesConfig,
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

    fn from_file(cdk_config_file: &PathBuf) -> OrchResult<Self> {
        let path = Path::new(&cdk_config_file);
        let cdk_config_file = File::open(path).map_err(|_err| OrchError::Init {
            dbg: format!("Scenario file not found: {:?}", path),
        })?;
        let config: CdkConfig = serde_json::from_reader(cdk_config_file).unwrap();
        Ok(config)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
struct CdkResourcesConfig {
    netbench_runner_log_group: String,
    netbench_runner_s3_bucket: String,
    netbench_cloudfront_distibution: String,
    netbench_runner_instance_profile: String,
    #[serde(rename(deserialize = "NetbenchRunnerVPCSubnetTag"))]
    netbench_runner_vpc_subnet_tag: CdkSubnetTagConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CdkSubnetTagConfig {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct HostConfig {
    pub az: String,
    instance_type: String,
    placement: PlacementGroupConfig,
}

impl HostConfig {
    fn new(az: String, placement: PlacementGroupConfig) -> Self {
        assert!(
            az.starts_with(STATE.region),
            "User specified AZ: {} is not in the region: {}",
            az,
            STATE.region
        );
        HostConfig {
            az,
            instance_type: "c5.4xlarge".to_owned(),
            placement,
        }
    }

    pub fn instance_type(&self) -> &String {
        &self.instance_type
    }

    pub fn to_ec2_placement(&self) -> AwsPlacement {
        let mut aws_placement = AwsPlacement::builder();

        // set placement group
        match self.placement {
            PlacementGroupConfig::Unspecified => {}
            PlacementGroupConfig::Cluster => {
                aws_placement = aws_placement.group_name(STATE.placement_group_cluster);
            }
        };

        // Set AZ. This is also set when chooing the subnet for the instance
        aws_placement = aws_placement.availability_zone(&self.az);

        aws_placement.build()
    }
}
