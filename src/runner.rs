/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(clippy::result_large_err)]
#![allow(dead_code)]
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_ec2 as ec2;
use aws_sdk_iam as iam;
use aws_sdk_sqs as sqs;
use base64::{engine::general_purpose, Engine as _};

#[derive(Debug)]
enum RunnerError {
    SQS(sqs::Error),
    EC2(ec2::Error),
    IAM(iam::Error),
    Str(&'static str),
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

///
/// ```rust
///     async fn launch(client: &aws_skd_ec2::Client, ) -> Result<Vec<String>, RunnerError>
/// ```
///
/// 1) Send my info to queue
/// 2) Runner gets info -> is ready for the run!
/// 3) Runner triggers the run
///
async fn launch(ec2_client: &ec2::Client, queue_url: &String) -> Result<Vec<String>, RunnerError> {
    let script = general_purpose::STANDARD_NO_PAD.encode(format!("aws sqs send-message --queue-url {queue_url} --message-body \"`ec2-metadata -i`\" --region us-west-2"));
    match ec2_client
        .run_instances()
        .min_count(1)
        .max_count(1)
        .dry_run(false)
        .image_id("ami-0efa651876de2a5ce")
        .user_data(script)
        .iam_instance_profile(
            ec2::types::IamInstanceProfileSpecification::builder()
                .name("NetbenchRunner")
                .build(),
        )
        .send()
        .await
    {
        Err(err) => {
            println!("Error: {:#?}", err);
            Err(())?
        }
        Ok(rio) => {
            println!("Ok: {:#?}", rio);
            Ok(rio
                .instances()
                .iter()
                .flat_map(|instances| {
                    instances
                        .iter()
                        .filter_map(|instance| instance.instance_id())
                })
                .map(|s| s.into())
                .collect())
        }
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

async fn new_instance_profile(client: &iam::Client) -> Result<iam::types::Role, RunnerError> {
    client
        .get_role()
        .role_name("*")
        .send()
        .await
        .map_err(|_| RunnerError::Str("No role found in region"))?
        .role()
        .cloned()
        .ok_or(RunnerError::Unit)
}

/// Sends a message to and receives the message from a queue in the Region.
#[tokio::main]
async fn main() -> Result<(), RunnerError> {
    tracing_subscriber::fmt::init();

    let region_provider = RegionProviderChain::default_provider().or_else("us-west-2");
    let shared_config = aws_config::from_env().region(region_provider).load().await;

    let ec2_client = ec2::Client::new(&shared_config);
    let sqs_client = sqs::Client::new(&shared_config);
    let iam_client = iam::Client::new(&shared_config);

    let role = new_instance_profile(&iam_client).await?;
    println!("got role: {:#?}", role);

    //let queue_url = find_first_queue(&sqs_client).await?;
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
