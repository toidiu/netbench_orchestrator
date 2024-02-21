// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::instance::{self, EndpointType, InstanceDetail},
    ec2_utils::networking,
    InfraDetail, OrchResult, OrchestratorConfig,
};
use std::time::Duration;
use tracing::debug;

#[derive(Clone)]
pub struct LaunchPlan<'a> {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub instance_profile_arn: String,
    pub config: &'a OrchestratorConfig,
}

impl<'a> LaunchPlan<'a> {
    pub async fn create(
        unique_id: &str,
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
        config: &'a OrchestratorConfig,
    ) -> Self {
        let instance_profile_arn = instance::get_instance_profile(iam_client)
            .await
            .expect("get_instance_profile failed");
        let ami_id = instance::get_latest_ami(ssm_client)
            .await
            .expect("get_latest_ami failed");

        let (subnet_id, vpc_id) = networking::get_subnet_vpc_ids(ec2_client)
            .await
            .expect("get_subnet_vpc_ids failed");
        // Create a security group
        let security_group_id = networking::create_security_group(ec2_client, &vpc_id, unique_id)
            .await
            .expect("create_security_group failed");

        LaunchPlan {
            ami_id,
            subnet_id,
            security_group_id,
            instance_profile_arn,
            config,
        }
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        let servers = instance::launch_instances(
            ec2_client,
            self,
            unique_id,
            &self.config,
            EndpointType::Server,
        )
        .await
        .map_err(|err| {
            debug!("{}", err);
            err
        })?;

        let clients = instance::launch_instances(
            ec2_client,
            self,
            unique_id,
            self.config,
            EndpointType::Client,
        )
        .await
        .map_err(|err| {
            debug!("{}", err);
            err
        })?;

        let mut infra = InfraDetail {
            security_group_id: self.security_group_id.clone(),
            clients: Vec::new(),
            servers: Vec::new(),
        };
        for (i, server) in servers.into_iter().enumerate() {
            let endpoint_type = EndpointType::Server;
            let server_ip = instance::poll_running(i, &endpoint_type, ec2_client, &server).await?;
            let server = InstanceDetail::new(endpoint_type, server, server_ip);
            infra.servers.push(server);
        }

        for (i, client) in clients.into_iter().enumerate() {
            let endpoint_type = EndpointType::Client;
            let client_ip = instance::poll_running(i, &endpoint_type, ec2_client, &client).await?;
            let client = InstanceDetail::new(endpoint_type, client, client_ip);
            infra.clients.push(client);
        }

        networking::configure_networking(ec2_client, &infra).await?;

        // wait for instance to spawn
        tokio::time::sleep(Duration::from_secs(50)).await;

        Ok(infra)
    }
}
