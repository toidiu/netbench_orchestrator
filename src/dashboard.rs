// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use tracing::info;
use crate::error::OrchResult;
use crate::upload_object;
use crate::InstanceDetail;
use crate::STATE;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;

pub enum Step<'a> {
    UploadIndex,
    ServerHostsRunning(&'a Vec<InstanceDetail>),
    ClientHostsRunning(&'a Vec<InstanceDetail>),
}

pub async fn update_dashboard(
    step: Step<'_>,
    s3_client: &aws_sdk_s3::Client,
    unique_id: &str,
) -> OrchResult<()> {
    match step {
        Step::UploadIndex => upload_index_html(s3_client, unique_id).await,
        Step::ServerHostsRunning(instances) => {
            update_instance_running(s3_client, instances, unique_id).await
        }
        Step::ClientHostsRunning(instances) => {
            update_instance_running(s3_client, instances, unique_id).await
        }
    }
}

async fn upload_index_html(s3_client: &aws_sdk_s3::Client, unique_id: &str) -> OrchResult<()> {
    let status = format!("{}/index.html", STATE.cf_url(unique_id));
    let template_server_prefix = format!("{}/server-step-", STATE.cf_url(unique_id));
    let template_client_prefix = format!("{}/client-step-", STATE.cf_url(unique_id));
    let template_finished_prefix = format!("{}/finished-step-", STATE.cf_url(unique_id));

    // Upload a status file to s3:
    let index_file = std::fs::read_to_string("index.html")
        .unwrap()
        .replace("template_unique_id", unique_id)
        .replace("template_server_prefix", &template_server_prefix)
        .replace("template_client_prefix", &template_client_prefix)
        .replace("template_finished_prefix", &template_finished_prefix);

    upload_object(
        s3_client,
        STATE.s3_log_bucket,
        ByteStream::from(Bytes::from(index_file)),
        &format!("{unique_id}/index.html"),
    )
    .await
    .unwrap();
    println!("Status: URL: {status}");
    info!("Status: URL: {status}");

    Ok(())
}

async fn update_instance_running(
    s3_client: &aws_sdk_s3::Client,
    instances: &[InstanceDetail],
    unique_id: &str,
) -> OrchResult<()> {
    let endpoint_type = &instances[0].endpoint_type.as_str();
    let mut instance_ip_id = String::new();
    instances.iter().for_each(|instance| {
        let id = instance.instance_id().unwrap();
        let string = format!("{} {}", instance.ip, id);
        instance_ip_id.push_str(&string);
    });

    upload_object(
        s3_client,
        STATE.s3_log_bucket,
        ByteStream::from(Bytes::from(format!(
            "EC2 {:?} instances up: {}",
            endpoint_type, instance_ip_id
        ))),
        // example: "unique_id/server-step-0"
        &format!("{unique_id}/{}-step-0", endpoint_type.to_lowercase()),
    )
    .await
    .unwrap();
    Ok(())
}
