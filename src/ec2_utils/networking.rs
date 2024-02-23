// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    error::{OrchError, OrchResult},
    InfraDetail, OrchestratorConfig, STATE,
};
use aws_sdk_ec2::types::{
    Filter, IpPermission, IpRange, ResourceType, TagSpecification, UserIdGroupPair,
};
use tracing::info;

pub async fn set_routing_permissions(
    ec2_client: &aws_sdk_ec2::Client,
    infra: &InfraDetail,
) -> OrchResult<()> {
    let sg_group = UserIdGroupPair::builder()
        .set_group_id(Some(infra.security_group_id.clone()))
        .build();

    // Egress
    ec2_client
        .authorize_security_group_egress()
        .group_id(infra.security_group_id.clone())
        .ip_permissions(
            // Authorize SG (all traffic within the same SG)
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .user_id_group_pairs(sg_group.clone())
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;

    let ssh_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    // TODO can we make this more restrictive?
    let russula_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    let public_host_ip_ranges: Vec<IpRange> = infra
        .clients
        .iter()
        .chain(infra.servers.iter())
        .map(|instance_detail| {
            info!(
                "{:?}: {} -- {}",
                instance_detail.endpoint_type,
                instance_detail.instance_id().expect("instance_id failed"),
                instance_detail.host_ips
            );

            IpRange::builder()
                .cidr_ip(format!("{}/32", instance_detail.host_ips.public_ip()))
                .build()
        })
        .collect();

    // Ingress
    ec2_client
        .authorize_security_group_ingress()
        .group_id(infra.security_group_id.clone())
        .ip_permissions(
            // Authorize SG (all traffic within the same SG)
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .user_id_group_pairs(sg_group)
                .build(),
        )
        .ip_permissions(
            // Authorize all host ips
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .set_ip_ranges(Some(public_host_ip_ranges.clone()))
                .build(),
        )
        .ip_permissions(
            // Authorize port 22 (ssh)
            IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(ssh_ip_range)
                .build(),
        )
        .ip_permissions(
            // Authorize russula ports (Coordinator <-> Workers)
            IpPermission::builder()
                .from_port(STATE.russula_port.into())
                .to_port(STATE.russula_port.into())
                .ip_protocol("tcp")
                .ip_ranges(russula_ip_range)
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct NetworkingInfraDetail {
    pub vpc_id: String,
    pub subnet_id: String,
}

pub async fn create_security_group(
    ec2_client: &aws_sdk_ec2::Client,
    vpc_id: &str,
    unique_id: &str,
) -> OrchResult<String> {
    let security_group_id = ec2_client
        .create_security_group()
        .group_name(STATE.security_group_name(unique_id))
        .description("This is a security group for a single run of netbench.")
        .vpc_id(vpc_id)
        .tag_specifications(
            TagSpecification::builder()
                .resource_type(ResourceType::SecurityGroup)
                .tags(
                    aws_sdk_ec2::types::Tag::builder()
                        .key("Name")
                        .value(STATE.security_group_name(unique_id))
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?
        .group_id()
        .expect("expected security_group_id")
        .into();
    Ok(security_group_id)
}

// TODO investigate if we should find a VPC and then its subnet
// Find or define the Subnet to Launch the Netbench Runners
//  - Default: Use the one defined by CDK
// Note: We may need to define more in different regions and AZ
//      There is some connection between Security Groups and
//      Subnets such that they have to be "in the same network"
//       I'm unclear here.
pub async fn get_subnet_vpc_ids(
    ec2_client: &aws_sdk_ec2::Client,
    config: &OrchestratorConfig,
    az: String,
) -> OrchResult<NetworkingInfraDetail> {
    let describe_subnet_output = ec2_client
        .describe_subnets()
        .filters(
            // Filter::builder()
            // .name(config.cdk_config.netbench_runner_vpc_subnet_tag_key())
            // .values(config.cdk_config.netbench_runner_vpc_subnet_tag_value())
            // .build(),
            Filter::builder().name("subnet-id").values("").build(),
        )
        .send()
        .await
        .map_err(|e| OrchError::Ec2 {
            dbg: format!("Couldn't describe subnets: {:#?}", e),
        })?;
    assert!(
        describe_subnet_output.subnets().expect("No subnets?").len() >= 1,
        "Couldn't describe subnets"
    );

    tracing::debug!("{:?}", describe_subnet_output.subnets());

    let subnet = &describe_subnet_output.subnets().expect("subnets failed")[0];
    let subnet_id = subnet.subnet_id().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find subnet".into(),
    })?;
    let vpc_id = subnet.vpc_id().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find vpc".into(),
    })?;
    Ok(NetworkingInfraDetail {
        vpc_id: vpc_id.to_owned(),
        subnet_id: subnet_id.to_owned(),
    })
}
