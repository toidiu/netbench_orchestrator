use crate::state::STATE;
use aws_sdk_ec2 as ec2;
use aws_sdk_ssm as ssm;
use ssm::operation::send_command::SendCommandOutput;
use std::{collections::HashMap, fmt::format, thread::sleep, time::Duration};

pub async fn wait_for_ssm_results(
    endpoint: &str,
    ssm_client: &ssm::Client,
    command_id: &str,
) -> bool {
    loop {
        let o_status = ssm_client
            .list_command_invocations()
            .command_id(command_id)
            .send()
            .await
            .unwrap()
            .command_invocations()
            .unwrap()
            .iter()
            .find_map(|command| command.status())
            .cloned();
        let status = match o_status {
            Some(s) => s,
            None => return true,
        };
        let dbg = format!("endpoint: {} status: {:?}", endpoint, status.clone());
        dbg!(dbg);

        match status {
            ssm::types::CommandInvocationStatus::Cancelled
            | ssm::types::CommandInvocationStatus::Cancelling
            | ssm::types::CommandInvocationStatus::Failed
            | ssm::types::CommandInvocationStatus::TimedOut => break false,
            ssm::types::CommandInvocationStatus::Delayed
            | ssm::types::CommandInvocationStatus::InProgress
            | ssm::types::CommandInvocationStatus::Pending => {
                sleep(Duration::from_secs(30));
                continue;
            }
            ssm::types::CommandInvocationStatus::Success => break true,
            _ => panic!("Unhandled Status"),
        };
    }
}

pub async fn send_command(
    _endpoint: &str,
    ssm_client: &ssm::Client,
    instance_id: &str,
    commands: Vec<String>,
) -> Option<SendCommandOutput> {
    let mut remaining_try_count: u32 = 30;
    loop {
        match ssm_client
            .send_command()
            .instance_ids(instance_id)
            .document_name("AWS-RunShellScript")
            .document_version("$LATEST")
            .parameters("commands", commands.clone())
            .cloud_watch_output_config(
                ssm::types::CloudWatchOutputConfig::builder()
                    .cloud_watch_log_group_name(STATE.cloud_watch_group)
                    .cloud_watch_output_enabled(true)
                    .build(),
            )
            .send()
            .await
            .map_err(|x| format!("{:#?}", x))
        {
            Ok(sent_command) => {
                break Some(sent_command);
            }
            Err(error_message) => {
                if remaining_try_count > 0 {
                    println!("Error message: {}", error_message);
                    println!("Trying again, waiting 30 seconds...");
                    sleep(Duration::new(30, 0));
                    remaining_try_count -= 1;
                    continue;
                } else {
                    return None;
                }
            }
        };
    }
}

// Find or define the Subnet to Launch the Netbench Runners
//  - Default: Use the one defined by CDK
// Note: We may need to define more in different regions and AZ
//      There is some connection between Security Groups and
//      Subnets such that they have to be "in the same network"
//       I'm unclear here.
pub async fn get_subnet_vpc_ids(
    ec2_client: &ec2::Client,
    subnet_name: &str,
) -> Result<(String, String), String> {
    let describe_subnet_output = ec2_client
        .describe_subnets()
        .filters(
            ec2::types::Filter::builder()
                .name("tag:aws-cdk:subnet-name")
                .values(subnet_name)
                .build(),
        )
        .send()
        .await
        .map_err(|e| format!("Couldn't describe subnets: {:#?}", e))?;
    assert_eq!(
        describe_subnet_output.subnets().expect("No subnets?").len(),
        1
    );
    let subnet_id = describe_subnet_output.subnets().unwrap()[0]
        .subnet_id()
        .ok_or::<String>("Couldn't find subnet".into())?;
    let vpc_id = describe_subnet_output.subnets().unwrap()[0]
        .vpc_id()
        .ok_or::<String>("Couldn't find subnet".into())?;
    Ok((subnet_id.into(), vpc_id.into()))
}
