use crate::ec2_utils::instance::launch_instance;
use crate::ec2_utils::instance::{EndpointType, InstanceDetail};
use crate::ec2_utils::poll_state;
use crate::error::{OrchError, OrchResult};
use crate::state::HostCount;
use crate::InfraDetail;
use crate::STATE;
use aws_sdk_ec2::types::{IpPermission, IpRange};

#[derive(Clone)]
pub struct LaunchPlan {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub instance_profile_arn: String,
    pub host_count: HostCount,
}

impl LaunchPlan {
    pub async fn create(
        unique_id: &str,
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
        host_count: HostCount,
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
            host_count,
        }
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        let server = format!("server-{}", unique_id);
        let client = format!("client-{}", unique_id);

        let mut servers = Vec::new();
        let mut clients = Vec::new();
        for _i in 0..self.host_count.servers {
            let server = launch_instance(ec2_client, self, &server).await?;
            servers.push(server);
        }
        for _i in 0..self.host_count.clients {
            let client = launch_instance(ec2_client, self, &client).await?;
            clients.push(client);
        }

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
                aws_sdk_ec2::types::InstanceStateName::Running,
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
                aws_sdk_ec2::types::InstanceStateName::Running,
            )
            .await?;

            let client = InstanceDetail::new(endpoint_type, client, client_ip);
            infra.clients.push(client);
        }

        configure_networking(ec2_client, &infra).await?;

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
            println!(
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

    let ssh_ip_range = aws_sdk_ec2::types::IpRange::builder()
        .cidr_ip("0.0.0.0/0")
        .build();

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
            aws_sdk_ec2::types::IpPermission::builder()
                .from_port(22)
                .to_port(22)
                .ip_protocol("tcp")
                .ip_ranges(ssh_ip_range)
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
        .group_name(STATE.sg_name_with_id(unique_id))
        .description("This is a security group for a single run of netbench.")
        .vpc_id(vpc_id)
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
            aws_sdk_ec2::types::Filter::builder()
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
