// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use self::instance::poll_state;
use crate::ec2_utils::instance::delete_instance;
use crate::error::{OrchError, OrchResult};
use std::{thread::sleep, time::Duration};
use tracing::info;

mod cluster;
mod instance;
mod launch_plan;

pub use instance::{EndpointType, InstanceDetail};
pub use launch_plan::LaunchPlan;

pub struct InfraDetail {
    pub security_group_id: String,
    pub clients: Vec<InstanceDetail>,
    pub servers: Vec<InstanceDetail>,
}

impl InfraDetail {
    pub async fn cleanup(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        self.delete_instances(ec2_client).await?;
        self.delete_security_group(ec2_client).await?;
        Ok(())
    }
}

impl InfraDetail {
    async fn delete_instances(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting instances");
        let ids: Vec<String> = self
            .servers
            .iter()
            .chain(self.clients.iter())
            .map(|instance| instance.instance_id().unwrap().to_string())
            .collect();

        delete_instance(ec2_client, ids).await?;
        Ok(())
    }

    async fn delete_security_group(&self, ec2_client: &aws_sdk_ec2::Client) -> OrchResult<()> {
        info!("Start: deleting security groups");
        let mut deleted_sec_group = ec2_client
            .delete_security_group()
            .group_id(self.security_group_id.to_string())
            .send()
            .await;
        sleep(Duration::from_secs(30));

        let mut retries = 10;
        while deleted_sec_group.is_err() && retries > 0 {
            sleep(Duration::from_secs(20));
            deleted_sec_group = ec2_client
                .delete_security_group()
                .group_id(self.security_group_id.to_string())
                .send()
                .await;

            retries -= 1;
        }

        deleted_sec_group.map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;

        Ok(())
    }
}
