// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{
        instance::{self, EndpointType, InstanceDetail},
        networking,
        networking::{Az, NetworkingInfraDetail, VpcId},
    },
    InfraDetail, OrchResult, OrchestratorConfig,
};
use std::time::Duration;
use tracing::debug;

#[derive(Clone, Debug)]
pub struct LaunchPlan<'a> {
    pub ami_id: String,
    pub networking_detail: NetworkingInfraDetail,
    pub vpc_id: VpcId,
    pub instance_profile_arn: String,
    pub config: &'a OrchestratorConfig,
}

impl<'a> LaunchPlan<'a> {
    pub async fn create(
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
        config: &'a OrchestratorConfig,
    ) -> Self {
        let instance_profile_arn = instance::get_instance_profile(iam_client, config)
            .await
            .expect("get_instance_profile failed");
        let ami_id = instance::get_latest_ami(ssm_client)
            .await
            .expect("get_latest_ami failed");
        let (networking_detail, vpc_id) = networking::get_subnet_vpc_ids(ec2_client, config)
            .await
            .unwrap();
        LaunchPlan {
            ami_id,
            networking_detail,
            vpc_id,
            instance_profile_arn,
            config,
        }
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        debug!("{:?}", self);

        let security_group_id =
            networking::create_security_group(ec2_client, &self.vpc_id, unique_id)
                .await
                .unwrap();

        let mut infra = InfraDetail {
            security_group_id,
            clients: Vec::new(),
            servers: Vec::new(),
        };

        // TODO the calls for server and client are similar.. dedupe into a function
        {
            let endpoint_type = EndpointType::Server;
            let mut launch_request = Vec::with_capacity(self.config.server_config.len());
            for host_config in &self.config.server_config {
                let server = instance::launch_instances(
                    ec2_client,
                    self,
                    &infra.security_group_id,
                    unique_id,
                    &host_config,
                    endpoint_type,
                )
                .await
                .map_err(|err| {
                    debug!("{}", err);
                    err
                });
                launch_request.push(server);
            }
            let launch_request: OrchResult<Vec<_>> = launch_request.into_iter().collect();
            // cleanup instances if a launch failed
            if let Err(launch_err) = launch_request {
                let _ = infra.cleanup(ec2_client).await.map_err(|delete_err| {
                    // ignore error on cleanup.. since this is best effort
                    debug!("{}", delete_err);
                });

                return Err(launch_err);
            }

            let launch_request = launch_request.unwrap();
            for (i, server) in launch_request.into_iter().enumerate() {
                let server_ip =
                    instance::poll_running(i, &endpoint_type, ec2_client, &server).await?;
                let az = server.placement().unwrap().availability_zone().unwrap();
                let server =
                    InstanceDetail::new(endpoint_type, Az::from(az.to_string()), server, server_ip);
                infra.servers.push(server);
            }
        }

        {
            let endpoint_type = EndpointType::Client;
            let mut launch_request = Vec::with_capacity(self.config.client_config.len());
            for host_config in &self.config.client_config {
                let client = instance::launch_instances(
                    ec2_client,
                    self,
                    &infra.security_group_id,
                    &unique_id,
                    &host_config,
                    endpoint_type,
                )
                .await
                .map_err(|err| {
                    debug!("{}", err);
                    err
                });
                launch_request.push(client);
            }

            let launch_request: OrchResult<Vec<_>> = launch_request.into_iter().collect();
            // cleanup instances if a launch failed
            if let Err(launch_err) = launch_request {
                let _ = infra.cleanup(ec2_client).await.map_err(|delete_err| {
                    // ignore error on cleanup.. since this is best effort
                    debug!("{}", delete_err);
                });

                return Err(launch_err);
            }

            let launch_request = launch_request.unwrap();
            for (i, client) in launch_request.into_iter().enumerate() {
                let client_ip =
                    instance::poll_running(i, &endpoint_type, ec2_client, &client).await?;
                let az = client.placement().unwrap().availability_zone().unwrap();
                let client =
                    InstanceDetail::new(endpoint_type, Az::from(az.to_string()), client, client_ip);
                infra.clients.push(client);
            }
        }

        networking::set_routing_permissions(ec2_client, &infra).await?;

        // wait for instance to spawn
        tokio::time::sleep(Duration::from_secs(10)).await;

        Ok(infra)
    }
}
