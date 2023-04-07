/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(clippy::result_large_err)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
use std::fmt::format;

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ec2 as ec2;
use aws_sdk_ec2instanceconnect as ec2ic;
use aws_sdk_iam as iam;
use aws_sdk_sqs as sqs;
use base64::{engine::general_purpose, Engine as _};
use ec2::types::Filter;

#[derive(Debug)]
enum RunnerError {
    SQS(sqs::Error),
    EC2(ec2::Error),
    IAM(iam::Error),
    Str(String),
    Unit,
}

impl From<ec2::Error> for RunnerError {
    fn from(err: ec2::Error) -> Self {
        Self::EC2(err)
    }
}

impl From<iam::Error> for RunnerError {
    fn from(err: iam::Error) -> Self {
        Self::IAM(err)
    }
}

impl From<sqs::Error> for RunnerError {
    fn from(err: sqs::Error) -> Self {
        Self::SQS(err)
    }
}

impl From<()> for RunnerError {
    fn from(_: ()) -> Self {
        Self::Unit
    }
}
async fn find_first_queue(client: &sqs::Client) -> Result<String, sqs::Error> {
    let queues = client.list_queues().send().await?;
    println!("{:#?}", queues);
    let queue_urls = queues.queue_urls().unwrap_or_default();
    assert!(
        queue_urls.len() <= 1,
        "Do you have the right creds? We found more than one queue."
    );
    Ok(queue_urls
        .first()
        .expect("No queues in this account and Region. Create a queue to proceed.")
        .to_string())
}

async fn get_security_groups(client: &ec2::Client) -> Option<String> {
    let sec_groups = client
        .describe_security_groups()
        .filters(
            ec2::types::Filter::builder()
                .name("description")
                .values("security group for the netbench runners")
                .build(),
        )
        .send()
        .await
        .expect("Hello");
    sec_groups
        .security_groups()
        .unwrap()
        .get(0)
        .expect("Couldn't find a sec group? are you authenticated?")
        .group_name()
        .map(|c| c.into())
}

///
/// ```rust
///     async fn launch(client: &aws_skd_ec2::Client, ) -> Result<Vec<String>, RunnerError>
/// ```
///
/// 1) Send my info to queue
/// 2) Runner gets info -> is ready for the run!
/// 3) Runner triggers the run
///
async fn launch(
    ec2_client: &ec2::Client,
    launch_template: ec2::types::LaunchTemplateSpecification,
    subnet_id: impl Into<String>,
) -> Result<Vec<ec2::types::Instance>, RunnerError> {
    match ec2_client
        .run_instances()
        .launch_template(launch_template)
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .subnet_id(subnet_id.into())
        .cpu_options(
            ec2::types::CpuOptionsRequest::builder()
                .core_count(8)
                .build(),
        )
        .send()
        .await
    {
        Err(err) => {
            println!("Error: {:#?}", err);
            Err(())?
        }
        Ok(rio) => Ok(rio.instances().unwrap().iter().map(|i| i.clone()).collect()),
    }
}

/*
 * describe_instance_status
 */
async fn wait_till_running(ec2_client: &ec2::Client, id: String) -> Result<(), RunnerError> {
    match ec2_client
        .describe_instance_status()
        .instance_ids(id)
        .send()
        .await
    {
        Err(err) => println!("Error: {:#?}", err),
        Ok(rio) => println!("Ok: {:#?}", rio),
    };
    Ok(())
}

async fn terminate(ec2_client: &ec2::Client, _: Vec<String>) -> () {
    match ec2_client
        .terminate_instances()
        .instance_ids("i-076063ab5a2cfb141")
        .instance_ids("i-0d77f31caff0420bd")
        .send()
        .await
    {
        Err(err) => println!("Error: {:#?}", err),
        Ok(rio) => println!("Ok: {:#?}", rio),
    }
}

async fn receive(client: &sqs::Client, queue_url: &String) -> Result<(), sqs::Error> {
    let rcv_message_output = client
        .receive_message()
        .max_number_of_messages(1)
        .visibility_timeout(60 * 1)
        .queue_url(queue_url)
        .send()
        .await?;

    println!("Messages from queue with url: {}", queue_url);

    let messages = rcv_message_output.messages.expect("No messages");
    assert!(
        messages.len() <= 1,
        "What happened? We set max_number_of_messages == 1"
    );
    let o_message = messages.get(0);
    println!("Got the message: {:#?}", o_message);

    if let Some(message) = o_message {
        client
            .delete_message()
            .receipt_handle(
                message
                    .receipt_handle
                    .as_ref()
                    .expect("How do we delete w/o a receipt handle?"),
            )
            .queue_url(queue_url)
            .send()
            .await?;
    }

    Ok(())
}

async fn get_instance_profile(client: &iam::Client) -> Result<String, RunnerError> {
    /*let role = client
        .get_role()
        .role_name("NetbenchRunner")
        .send()
        .await
        .map_err(|a| RunnerError::Str(format!("{:#?}", a)))?
        .role()
        .cloned()
        .expect("role failed");
    */

    let instance_profile = client
        .get_instance_profile()
        .instance_profile_name("NetbenchRunnerInstanceProfile")
        .send()
        .await
        .map_err(|a| RunnerError::Str(format!("{:#?}", a)))?
        .instance_profile()
        .expect("ip failed")
        .clone();
    /*
    client.add_role_to_instance_profile()
        .instance_profile_name("NetbenchRunnerInstanceProfile1").role_name("NetbenchRunner").send().await.expect("Couldn't add role to instance profile?");
    */

    instance_profile
        .arn()
        .map(str::to_string)
        .ok_or(RunnerError::Str("No instance profile found".into()))
}

/// Sends a message to and receives the message from a queue in the Region.
#[tokio::main]
async fn main() -> Result<(), RunnerError> {
    tracing_subscriber::fmt::init();

    let region_provider = RegionProviderChain::default_provider().or_else("us-west-2");
    // print!("{:#?}", region_provider);
    let shared_config = aws_config::from_env().region(region_provider).load().await;

    let ec2_client = ec2::Client::new(&shared_config);
    let sqs_client = sqs::Client::new(&shared_config);
    let iam_client = iam::Client::new(&shared_config);
    let ec2ic_client = ec2ic::Client::new(&shared_config);

    // 1. Get the launch template
    let launch_templates: Vec<String> = ec2_client
        .describe_launch_templates()
        .launch_template_names("NetbenchRunnerTemplate")
        .send()
        .await
        .expect("No launch template?")
        .launch_templates()
        .expect("No launch templates? 2")
        .iter()
        .map(|lt| lt.launch_template_name().unwrap().into())
        .collect();

    assert_eq!(launch_templates.len(), 1);
    let launch_template = launch_templates.get(0).unwrap();
    print!("Got a launch template: {}", launch_template);

    // 2. Get SQS queue. -- might not need this in the end -- move to launch template?
    //let queue_url = find_first_queue(&sqs_client).await?;

    // 3. Get VPC
    let x = ec2_client
        .describe_subnets()
        .filters(
            /* .values("public-subnet-runners-1") */
            ec2::types::Filter::builder()
                .name("tag:aws-cdk:subnet-name")
                .values("public-subnet-runners-1")
                .build(),
        )
        .send()
        .await
        .expect("error");
    assert_eq!(x.subnets().expect("None?").len(), 1);
    let subnet_id = x.subnets().unwrap()[0].subnet_id().expect("No vpc id?");
    println!("Subnets\n{:#?}", subnet_id);

    // 3. Launch w/ Launch Template
    let instances = launch(
        &ec2_client,
        ec2::types::builders::LaunchTemplateSpecificationBuilder::default()
            .launch_template_name(launch_template)
            .build(),
        subnet_id,
    )
    .await?;

    println!("{:#?}", instances);

    let instance_ids: Vec<String> = instances
        .iter()
        .map(|instance| instance.instance_id().unwrap().into())
        .collect();

    let public_key = include_str!("id_rsa.pub");
    let private_key = include_str!("id_rsa");

    for id in instance_ids.iter() {
        loop {
            let result = ec2ic_client
                .send_ssh_public_key()
                .instance_os_user("ec2-user")
                .ssh_public_key(public_key)
                .instance_id(id)
                .send()
                .await;
            if result.is_ok() {
                break;
            } else {
            }
        }
    }
    //launch(&ec2_client, &queue_url, instance_profile).await?;

    //println!("{}", queue_url);
    //launch(&ec2_client, &queue_url).await?;
    //receive(&client, &queue_url).await?;
    //let ids = launch(&ec2_client).await?;
    //println!("{:#?}", ids);
    //[
    //"i-0502b697fe8b5816b",
    //"i-080ddbf19b0bb0daf",
    //]

    //wait_till_running(&ec2_client, "i-080ddbf19b0bb0daf".into()).await?;
    //terminate(&ec2_client, vec![]).await;

    /*
     *
     */
    //new_instance_profile(client);
    Ok(())
}
