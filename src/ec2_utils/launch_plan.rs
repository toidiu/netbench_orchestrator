use crate::ec2_utils::instance::get_subnet_vpc_ids;
use crate::ec2_utils::instance::launch_instance;
use crate::ec2_utils::instance::{EndpointType, InstanceDetail};
use crate::ec2_utils::poll_state;
use crate::error::{OrchError, OrchResult};
use crate::InfraDetail;
use aws_sdk_ec2::types::{IpPermission, IpRange};

#[derive(Clone)]
pub struct LaunchPlan {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub iam_role: String,
}

impl LaunchPlan {
    pub async fn create(
        unique_id: &str,
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &aws_sdk_iam::Client,
        ssm_client: &aws_sdk_ssm::Client,
    ) -> Self {
        let iam_role: String = iam_client
            .get_instance_profile()
            .instance_profile_name("NetbenchRunnerInstanceProfile")
            .send()
            .await
            .unwrap()
            .instance_profile()
            .unwrap()
            .arn()
            .unwrap()
            .into();

        // Find or define the Subnet to Launch the Netbench Runners
        let (subnet_id, vpc_id) =
            get_subnet_vpc_ids(ec2_client, "public-subnet-for-runners-in-us-east-1")
                .await
                .unwrap();

        // Create a security group
        let security_group_id: String = ec2_client
            .create_security_group()
            .group_name(format!("generated_group_{}", unique_id))
            .description("This is a security group for a single run of netbench.")
            .vpc_id(vpc_id)
            .send()
            .await
            .expect("No output?")
            .group_id()
            .expect("No group ID?")
            .into();

        // Get latest ami
        let ami_id: String = ssm_client
            .get_parameter()
            .name("/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64")
            .with_decryption(true)
            .send()
            .await
            .unwrap()
            .parameter()
            .unwrap()
            .value()
            .unwrap()
            .into();

        LaunchPlan {
            ami_id,
            subnet_id,
            security_group_id,
            iam_role,
        }
    }

    pub async fn launch(
        &self,
        ec2_client: &aws_sdk_ec2::Client,
        unique_id: &str,
    ) -> OrchResult<InfraDetail> {
        let server = format!("server-{}", unique_id);
        let client = format!("client-{}", unique_id);

        let server = launch_instance(ec2_client, self, &server).await?;
        let client = launch_instance(ec2_client, self, &client).await?;

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
            self.security_group_id.clone(),
        );
        let client = InstanceDetail::new(
            EndpointType::Client,
            client,
            client_ip,
            self.security_group_id.clone(),
        );

        let infra = InfraDetail {
            security_group_id: self.security_group_id.clone(),
            clients: vec![client],
            server: vec![server],
        };
        let client = infra.clients.get(0).unwrap();
        let server = infra.server.get(0).unwrap();

        configure_networking(ec2_client, client, server).await?;

        println!(
            "client: {} server: {} \n client_ip: {} \nserver_ip: {}",
            client.instance_id()?,
            server.instance_id()?,
            client.ip,
            server.ip
        );

        Ok(infra)
    }
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
