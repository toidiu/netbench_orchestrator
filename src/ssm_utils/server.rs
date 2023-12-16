// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::state::STATE;
use aws_sdk_ssm::operation::send_command::SendCommandOutput;

pub async fn run_netbench(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command(vec![Step::BuildNetbench], Step::RunNetbench, "client", "run_client_netbench", ssm_client, instance_ids, vec![
        "cd s2n-quic/netbench",
        format!("env COORD_CLIENT_0={}:8080 ./scripts/netbench-test-player-as-server.sh", client_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-7", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench {}", STATE.s3_path(unique_id)).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html {}/server-step-8", STATE.s3_path(unique_id)).as_str(),
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}
