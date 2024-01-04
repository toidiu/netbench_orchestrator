// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::s3_utils::*;
use crate::state::*;
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use std::process::Command;

pub async fn orch_generate_report(s3_client: &aws_sdk_s3::Client, unique_id: &str) {
    // download results from s3 -----------------------
    let mut cmd = Command::new("aws");
    cmd.args([
        "s3",
        "sync",
        &format!("s3://{}/{}", STATE.s3_log_bucket, unique_id),
        STATE.workspace_dir,
    ]);
    println!("{:?}", cmd);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");

    // CLI ---------------------------
    let results_path = format!("{}/results", STATE.workspace_dir);
    let report_path = format!("{}/report", STATE.workspace_dir);
    let mut cmd = Command::new("s2n-netbench");
    cmd.args(["report-tree", &results_path, &report_path]);
    println!("{:?}", cmd);
    let status = cmd.status().expect("s2n-netbench command failed");
    assert!(status.success(), " s2n-netbench command failed");

    // upload report to s3 -----------------------
    let mut cmd = Command::new("aws");
    cmd.args([
        "s3",
        "sync",
        STATE.workspace_dir,
        &format!("s3://{}/{}", STATE.s3_log_bucket, unique_id),
    ]);
    println!("{:?}", cmd);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");

    update_report_url(s3_client, unique_id).await;

    println!("Report Finished!: Successful: true");
    println!("URL: {}/report/index.html", STATE.cf_url(unique_id));
}

async fn update_report_url(s3_client: &aws_sdk_s3::Client, unique_id: &str) {
    let body = ByteStream::new(SdkBody::from(format!(
        "<a href=\"{}/report/index.html\">Final Report</a>",
        STATE.cf_url(unique_id)
    )));
    let key = format!("{}/finished-step-0", unique_id);
    let _ = upload_object(s3_client, STATE.s3_log_bucket, body, &key)
        .await
        .unwrap();
}
