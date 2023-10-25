use self::instance::poll_state;
use crate::error::{OrchError, OrchResult};
use aws_sdk_ec2::types::{IpPermission, IpRange};
use std::{thread::sleep, time::Duration};

mod cluster;
mod instance;
mod launch_plan;

pub use launch_plan::LaunchPlan;

enum EndpointType {
    Server,
    Client,
}

pub struct InstanceDetail {
    endpoint_type: EndpointType,
    pub instance: aws_sdk_ec2::types::Instance,
    pub ip: String,
    pub security_group_id: String,
}

impl InstanceDetail {
    fn new(
        endpoint_type: EndpointType,
        instance: aws_sdk_ec2::types::Instance,
        ip: String,
        security_group_id: String,
    ) -> Self {
        InstanceDetail {
            endpoint_type,
            instance,
            ip,
            security_group_id,
        }
    }

    pub fn instance_id(&self) -> OrchResult<&str> {
        self.instance.instance_id().ok_or(OrchError::Ec2 {
            dbg: "No client id".to_string(),
        })
    }
}

pub async fn launch_server_client(
    ec2_client: &aws_sdk_ec2::Client,
    instance_details: &LaunchPlan,
    unique_id: &str,
) -> OrchResult<(InstanceDetail, InstanceDetail)> {
    let server = format!("server-{}", unique_id);
    let client = format!("client-{}", unique_id);

    let server = instance::launch_instance(ec2_client, instance_details, &server).await?;
    let client = instance::launch_instance(ec2_client, instance_details, &client).await?;

    let server_ip = poll_state(
        ec2_client,
        &server,
        aws_sdk_ec2::types::InstanceStateName::Running,
    )
    .await?;
    let client_ip = poll_state(
        ec2_client,
        &client,
        aws_sdk_ec2::types::InstanceStateName::Running,
    )
    .await?;

    let server = InstanceDetail::new(
        EndpointType::Server,
        server,
        server_ip,
        instance_details.security_group_id.clone(),
    );
    let client = InstanceDetail::new(
        EndpointType::Client,
        client,
        client_ip,
        instance_details.security_group_id.clone(),
    );

    configure_networking(ec2_client, &client, &server).await?;

    println!(
        "client: {} server: {} \n client_ip: {} \nserver_ip: {}",
        client.instance_id()?,
        server.instance_id()?,
        client.ip,
        server.ip
    );

    Ok((server, client))
}

async fn configure_networking(
    ec2_client: &aws_sdk_ec2::Client,
    client: &InstanceDetail,
    server: &InstanceDetail,
) -> OrchResult<()> {
    ec2_client
        .authorize_security_group_egress()
        .group_id(&client.security_group_id)
        .ip_permissions(
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    IpRange::builder()
                        .cidr_ip(format!("{}/32", client.ip))
                        .build(),
                )
                .ip_ranges(
                    IpRange::builder()
                        .cidr_ip(format!("{}/32", server.ip))
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;
    ec2_client
        .authorize_security_group_ingress()
        .group_id(&client.security_group_id)
        .ip_permissions(
            IpPermission::builder()
                .from_port(-1)
                .to_port(-1)
                .ip_protocol("-1")
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", client.ip))
                        .build(),
                )
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip(format!("{}/32", server.ip))
                        .build(),
                )
                .build(),
        )
        .ip_permissions(
            aws_sdk_ec2::types::IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(
                    aws_sdk_ec2::types::IpRange::builder()
                        .cidr_ip("0.0.0.0/0")
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .map_err(|err| OrchError::Ec2 {
            dbg: err.to_string(),
        })?;

    Ok(())
}

pub async fn delete_security_group(ec2_client: aws_sdk_ec2::Client, security_group_id: &str) {
    println!("Start: deleting security groups");
    let mut deleted_sec_group = ec2_client
        .delete_security_group()
        .group_id(security_group_id)
        .send()
        .await;
    sleep(Duration::from_secs(60));

    while deleted_sec_group.is_err() {
        sleep(Duration::from_secs(30));
        deleted_sec_group = ec2_client
            .delete_security_group()
            .group_id(security_group_id)
            .send()
            .await;
    }
    println!("Deleted Security Group: {:#?}", deleted_sec_group);
    println!("Done: deleting security groups");
}
