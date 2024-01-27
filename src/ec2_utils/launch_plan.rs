// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{
        instance::{launch_instance, EndpointType, InstanceDetail},
        poll_state,
    },
    error::{OrchError, OrchResult},
    InfraDetail, Scenario, STATE,
};
use aws_sdk_ec2::types::{
    Filter, InstanceStateName, IpPermission, IpRange, ResourceType, TagSpecification,
};
use std::time::Duration;
use tracing::info;

#[derive(Clone)]
pub struct LaunchPlan<'a> {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub instance_profile_arn: String,
    pub scenario: &'a Scenario,
}

impl<'a> LaunchPlan<'a> {
    pub async fn create(
        unique_id: &str,
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
        scenario: &'a Scenario,
    ) -> Self {
        let instance_profile_arn = get_instance_profile(iam_client).await.unwrap();
        let (subnet_id, vpc_id) = get_subnet_vpc_ids(ec2_client).await.unwrap();
        let ami_id = get_latest_ami(ssm_client).await.unwrap();
        // Create a security group
        let security_group_id = create_security_group(ec2_client, &vpc_id, unique_id)
            .await
            .unwrap();

        LaunchPlan {
            ami_id,
            subnet_id,
            security_group_id,
            instance_profile_arn,
            scenario,
        }
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        let servers = launch_instance(
            ec2_client,
            self,
            unique_id,
            self.scenario.servers,
            EndpointType::Server,
        )
        .await?;

        let clients = launch_instance(
            ec2_client,
            self,
            unique_id,
            self.scenario.clients,
            EndpointType::Client,
        )
        .await?;

        let mut infra = InfraDetail {
            security_group_id: self.security_group_id.clone(),
            clients: Vec::new(),
            servers: Vec::new(),
        };
        for (i, server) in servers.into_iter().enumerate() {
            let endpoint_type = EndpointType::Server;
            let server_ip = poll_state(
                i,
                &endpoint_type,
                ec2_client,
                &server,
                InstanceStateName::Running,
            )
            .await?;

            let server = InstanceDetail::new(endpoint_type, server, server_ip);
            infra.servers.push(server);
        }

        for (i, client) in clients.into_iter().enumerate() {
            let endpoint_type = EndpointType::Client;
            let client_ip = poll_state(
                i,
                &endpoint_type,
                ec2_client,
                &client,
                InstanceStateName::Running,
            )
            .await?;

            let client = InstanceDetail::new(endpoint_type, client, client_ip);
            infra.clients.push(client);
        }

        configure_networking(ec2_client, &infra).await?;

        // wait for instance to spawn
        tokio::time::sleep(Duration::from_secs(50)).await;

        Ok(infra)
    }
}

async fn configure_networking(
    ec2_client: &aws_sdk_ec2::Client,
    infra: &InfraDetail,
) -> OrchResult<()> {
    let host_ip_ranges: Vec<IpRange> = infra
        .clients
        .iter()
        .chain(infra.servers.iter())
        .map(|instance_detail| {
            info!(
                "{:?}: {} -- {}",
                instance_detail.endpoint_type,
                instance_detail.instance_id().unwrap(),
                instance_detail.ip
            );

            IpRange::builder()
                .cidr_ip(format!("{}/32", instance_detail.ip))
                .build()
        })
        .collect();

    let ssh_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();
    // TODO can we make this more restrictive?
    let russula_ip_range = IpRange::builder().cidr_ip("0.0.0.0/0").build();

    ec2_client
        .authorize_security_group_egress()
        .group_id(infra.security_group_id.clone())
        .ip_permissions(
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .set_ip_ranges(Some(host_ip_ranges.clone()))
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;
    ec2_client
        .authorize_security_group_ingress()
        .group_id(infra.security_group_id.clone())
        .ip_permissions(
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .set_ip_ranges(Some(host_ip_ranges.clone()))
                .build(),
        )
        .ip_permissions(
            IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(ssh_ip_range)
                .build(),
        )
        .ip_permissions(
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

async fn create_security_group(
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

async fn get_instance_profile(iam_client: &aws_sdk_iam::Client) -> OrchResult<String> {
    let instance_profile_arn = iam_client
        .get_instance_profile()
        .instance_profile_name(STATE.instance_profile)
        .send()
        .await
        .map_err(|err| OrchError::Iam {
            dbg: err.to_string(),
        })?
        .instance_profile()
        .unwrap()
        .arn()
        .unwrap()
        .into();
    Ok(instance_profile_arn)
}

async fn get_latest_ami(ssm_client: &aws_sdk_ssm::Client) -> OrchResult<String> {
    let ami_id = ssm_client
        .get_parameter()
        .name("/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64")
        .with_decryption(true)
        .send()
        .await
        .map_err(|err| OrchError::Ssm {
            dbg: err.to_string(),
        })?
        .parameter()
        .expect("expected ami value")
        .value()
        .expect("expected ami value")
        .into();
    Ok(ami_id)
}

// TODO investigate if we should find a VPC and then its subnet
// Find or define the Subnet to Launch the Netbench Runners
//  - Default: Use the one defined by CDK
// Note: We may need to define more in different regions and AZ
//      There is some connection between Security Groups and
//      Subnets such that they have to be "in the same network"
//       I'm unclear here.
async fn get_subnet_vpc_ids(ec2_client: &aws_sdk_ec2::Client) -> OrchResult<(String, String)> {
    let describe_subnet_output = ec2_client
        .describe_subnets()
        .filters(
            Filter::builder()
                .name(STATE.subnet_tag_value.0)
                .values(STATE.subnet_tag_value.1)
                .build(),
        )
        .send()
        .await
        .map_err(|e| OrchError::Ec2 {
            dbg: format!("Couldn't describe subnets: {:#?}", e),
        })?;
    assert_eq!(
        describe_subnet_output.subnets().expect("No subnets?").len(),
        1
    );

    let subnet = &describe_subnet_output.subnets().unwrap()[0];
    let subnet_id = subnet.subnet_id().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find subnet".into(),
    })?;
    let vpc_id = subnet.vpc_id().ok_or(OrchError::Ec2 {
        dbg: "Couldn't find vpc".into(),
    })?;
    Ok((subnet_id.into(), vpc_id.into()))
}
