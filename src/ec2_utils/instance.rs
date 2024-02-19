// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::{OrchError, OrchResult},
    state::STATE,
    LaunchPlan,
};
use aws_sdk_ec2::types::{
    BlockDeviceMapping, EbsBlockDevice, IamInstanceProfileSpecification, Instance,
    InstanceNetworkInterfaceSpecification, InstanceStateName, InstanceType, ResourceType,
    ShutdownBehavior, Tag, TagSpecification,
};
use base64::{engine::general_purpose, Engine as _};
use std::{net::IpAddr, str::FromStr, time::Duration};
use tracing::info;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PubIp(pub IpAddr);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PrivIp(pub IpAddr);

impl std::fmt::Display for PrivIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for PubIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct HostIps {
    private_ip: PrivIp,
    public_ip: PubIp,
}

impl HostIps {
    pub fn public_ip(&self) -> &PubIp {
        &self.public_ip
    }

    pub fn private_ip(&self) -> &PrivIp {
        &self.private_ip
    }
}

impl std::fmt::Display for HostIps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "public_ip: {}, private_ip: {}",
            self.public_ip, self.private_ip
        )
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum EndpointType {
    Server,
    Client,
}

impl EndpointType {
    pub fn as_str(&self) -> &str {
        match self {
            EndpointType::Server => "Server",
            EndpointType::Client => "Client",
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct InstanceDetail {
    pub endpoint_type: EndpointType,
    pub instance_id: String,
    pub host_ips: HostIps,
}

impl InstanceDetail {
    pub fn new(endpoint_type: EndpointType, instance: Instance, host_ips: HostIps) -> Self {
        let instance_id = instance
            .instance_id()
            .ok_or(OrchError::Ec2 {
                dbg: "No instance id".to_string(),
            })
            .expect("instance_id failed")
            .to_string();

        InstanceDetail {
            endpoint_type,
            instance_id,
            host_ips,
        }
    }

    pub fn instance_id(&self) -> OrchResult<&str> {
        Ok(&self.instance_id)
    }
}

pub async fn launch_instance(
    ec2_client: &aws_sdk_ec2::Client,
    launch_plan: &LaunchPlan<'_>,
    unique_id: &str,
    count: usize,
    endpoint_type: EndpointType,
) -> OrchResult<Vec<Instance>> {
    let instance_type = InstanceType::from(STATE.instance_type);
    let run_result = ec2_client
        .run_instances()
        .key_name(STATE.ssh_key_name)
        .iam_instance_profile(
            IamInstanceProfileSpecification::builder()
                .arn(&launch_plan.instance_profile_arn)
                .build(),
        )
        .instance_type(instance_type)
        .image_id(&launch_plan.ami_id)
        .instance_initiated_shutdown_behavior(ShutdownBehavior::Terminate)
        .user_data(
            general_purpose::STANDARD.encode(format!("sudo shutdown -P +{}", STATE.shutdown_min)),
        )
        // give the instances human readable names. name is set via tags
        .tag_specifications(
            TagSpecification::builder()
                .resource_type(ResourceType::Instance)
                .tags(
                    Tag::builder()
                        .key("Name")
                        .value(STATE.instance_name(unique_id, endpoint_type))
                        .build(),
                )
                .build(),
        )
        .block_device_mappings(
            BlockDeviceMapping::builder()
                .device_name("/dev/xvda")
                .ebs(
                    EbsBlockDevice::builder()
                        .delete_on_termination(true)
                        .volume_size(50)
                        .build(),
                )
                .build(),
        )
        .network_interfaces(
            InstanceNetworkInterfaceSpecification::builder()
                .associate_public_ip_address(true)
                .delete_on_termination(true)
                .device_index(0)
                .subnet_id(&launch_plan.subnet_id)
                .groups(&launch_plan.security_group_id)
                .build(),
        )
        .min_count(count as i32)
        .max_count(count as i32)
        .dry_run(false)
        .send()
        .await
        .map_err(|r| OrchError::Ec2 {
            dbg: format!("{:#?}", r),
        })?;
    let instances = run_result.instances().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find instances in run result".to_string(),
    })?;

    Ok(instances.to_vec())
}

pub async fn delete_instance(ec2_client: &aws_sdk_ec2::Client, ids: Vec<String>) -> OrchResult<()> {
    ec2_client
        .terminate_instances()
        .set_instance_ids(Some(ids))
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;
    Ok(())
}

pub async fn poll_running(
    enumerate: usize,
    endpoint_type: &EndpointType,
    ec2_client: &aws_sdk_ec2::Client,
    instance: &Instance,
) -> OrchResult<HostIps> {
    // Wait for running state
    let mut actual_state = InstanceStateName::Pending;
    let mut host_ip = None;
    while actual_state != InstanceStateName::Running {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let result = ec2_client
            .describe_instances()
            .instance_ids(instance.instance_id().expect("describe_instances failed"))
            .send()
            .await
            .expect("ec2 send failed");
        let res = result.reservations().expect("reservations failed");

        let inst = res
            .get(0)
            .expect("reservations get(0) failed")
            .instances()
            .expect("instances failed")
            .get(0)
            .expect("instances get(0) failed");

        host_ip = inst
            .private_ip_address()
            .and_then(|ip| IpAddr::from_str(ip).ok())
            .map(|private_ip| {
                inst.public_ip_address()
                    .and_then(|ip| IpAddr::from_str(ip).ok())
                    .map(|public_ip| HostIps {
                        private_ip: PrivIp(private_ip),
                        public_ip: PubIp(public_ip),
                    })
            })
            .flatten();

        actual_state = inst
            .state()
            .expect("state failed")
            .name()
            .expect("name failed")
            .clone();

        info!(
            "{:?} {} state: {:?}",
            endpoint_type, enumerate, actual_state
        );
    }

    host_ip.ok_or(OrchError::Ec2 {
        dbg: "".to_string(),
    })
}
