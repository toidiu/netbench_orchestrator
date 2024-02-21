// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use aws_sdk_ec2::types::Placement as AwsPlacement;
use clap::Args;
use clap::Parser;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Cli {
    /// Path to the scenario file
    #[arg(long)]
    pub scenario_file: PathBuf,

    #[command(flatten)]
    pub infra: InfraScenario,
}

#[derive(Copy, Clone, Debug, Default, Args)]
pub struct InfraScenario {
    /// Placement strategy for the netbench hosts
    #[arg(long, default_value="cluster")]
    pub placement: PlacementGroup,

    // #[arg(long)]
    // instance_type: String
    // region
    // AZ
    // ssh_key_name
}

impl From<PlacementGroup> for AwsPlacement {
    fn from(value: PlacementGroup) -> Self {
        let mut placement = AwsPlacement::builder();
        placement = match value {
            PlacementGroup::Cluster => placement.group_name(STATE.placement_group_cluster),
        };
        placement.build()
    }
}

// https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/placement-groups.html?icmpid=docs_ec2_console
#[derive(Copy, Clone, Debug, Default, clap::ValueEnum)]
pub enum PlacementGroup {
    #[default]
    // Packs instances close together inside an Availability Zone. This
    // strategy enables workloads to achieve the low-latency network
    // performance necessary for tightly-coupled node-to-node communication
    // that is typical of high-performance computing (HPC) applications.
    Cluster,

    // support partition
    // // Spreads your instances across logical partitions such that groups of
    // // instances in one partition do not share the underlying hardware with
    // // groups of instances in different partitions. This strategy is
    // // typically used by large distributed and replicated workloads, such as
    // // Hadoop, Cassandra, and Kafka.
    // Partition,

    // TODO support spread
    // // Strictly places a small group of instances across distinct underlying
    // // hardware to reduce correlated failures.
    // Spread,
}

// Used for parsing the scenario file generated by the s2n-netbench project
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NetbenchScenario {
    // pub id: Id,
    pub clients: Vec<Value>,
    pub servers: Vec<Value>,

    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub routers: Vec<Arc<Router>>,
    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub traces: Arc<Vec<String>>,
    // #[serde(skip_serializing_if = "Vec::is_empty", default)]
    // pub certificates: Vec<Arc<Certificate>>,
}

#[derive(Clone, Debug)]
pub struct OrchestratorScenario {
    pub netbench_scenario_filename: String,
    pub netbench_scenario_filepath: PathBuf,
    pub clients: usize,
    pub servers: usize,
}

impl OrchestratorScenario {
    pub fn netbench_scenario_file_stem(&self) -> &str {
        self.netbench_scenario_filepath
            .as_path()
            .file_stem()
            .expect("expect scenario file")
            .to_str()
            .unwrap()
    }
}
