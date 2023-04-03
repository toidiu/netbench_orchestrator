/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0.
 */
#![allow(clippy::result_large_err)]

fn main() {}
/*
use aws_config::meta::region::ProvideRegion;
use aws_sdk_sqs::{Region, PKG_VERSION, Client, Error};

async fn find_first_queue(client: &Client) -> Result<String, Error> {
    let queues = client.list_queues().send().await?;
    let queue_urls = queues.queue_urls().unwrap_or_default();
    Ok(queue_urls
        .first()
        .expect("No queues in this account and Region. Create a queue to proceed.")
        .to_string())
}

// Send a message to a queue.
async fn send(client: &Client, queue_url: &String, message: &String) -> Result<(), Error> {
    println!("Sending message to queue with URL: {}", queue_url);

    let rsp = client
        .send_message()
        .queue_url(queue_url)
        .message_body(message)
        // .message_group_id(&message.group)
        // .message_deduplication_id("a-unique-id-for-dedup")
        // If the queue is FIFO, you need to set .message_deduplication_id
        // or configure the queue for ContentBasedDeduplication.
        .send()
        .await?;

    println!("Send message to the queue: {:#?}", rsp);

    Ok(())
}

// Pump a queue for up to 10 outstanding messages.
async fn receive(client: &Client, queue_url: &String) -> Result<(), Error> {
    let rcv_message_output = client.receive_message().queue_url(queue_url).send().await?;

    println!("Messages from queue with url: {}", queue_url);

    for message in rcv_message_output.messages.unwrap_or_default() {
        println!("Got the message: {:#?}", message);
    }

    Ok(())
}

/// Sends a message to and receives the message from a queue in the Region.
#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();
    let queue = None;
    let verbose = false;
    let region_provider = Region::new("us-west-2");

    println!();
    if verbose {
        println!("SQS client version: {}", PKG_VERSION);
        println!(
            "Region:             {}",
            region_provider.region().await.unwrap().as_ref()
        );
        println!();
    }

    let shared_config = aws_config::from_env().region(region_provider).load().await;
    let client = Client::new(&shared_config);
    let first_queue_url = find_first_queue(&client).await?;
    let queue_url = queue.unwrap_or(first_queue_url);

    let message = "hello from my queue".to_owned();

    send(&client, &queue_url, &message).await?;
    // receive(&client, &queue_url).await?;

    Ok(())
}
*/
