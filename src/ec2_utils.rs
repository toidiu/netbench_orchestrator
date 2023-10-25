use self::instance::poll_state;
use crate::ec2_utils::instance::InstanceDetail;
use std::{thread::sleep, time::Duration};

mod cluster;
mod instance;
mod launch_plan;

pub use launch_plan::LaunchPlan;

pub struct InfraDetail {
    pub security_group_id: String,
    pub clients: Vec<InstanceDetail>,
    pub server: Vec<InstanceDetail>,
}

impl InfraDetail {
    pub async fn cleanup(&self, ec2_client: &aws_sdk_ec2::Client) {
        delete_security_group(ec2_client, &self.security_group_id).await;
    }
}

async fn delete_security_group(ec2_client: &aws_sdk_ec2::Client, security_group_id: &str) {
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
