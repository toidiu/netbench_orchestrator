use crate::ec2_utils::instance::get_subnet_vpc_ids;
use aws_sdk_iam as iam;
use aws_sdk_ssm as ssm;

#[derive(Clone)]
pub struct LaunchPlan {
    pub subnet_id: String,
    pub security_group_id: String,
    pub ami_id: String,
    pub iam_role: String,
}

impl LaunchPlan {
    pub async fn new(
        unique_id: &str,
        ec2_client: &aws_sdk_ec2::Client,
        iam_client: &iam::Client,
        ssm_client: &ssm::Client,
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
}
