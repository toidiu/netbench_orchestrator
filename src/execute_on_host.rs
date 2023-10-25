use crate::s3_helper::*;
use crate::state::*;
use crate::utils::*;
use aws_sdk_s3::primitives::{ByteStream, SdkBody};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::process::Command;

pub async fn execute_ssm_client(
    ssm_client: &aws_sdk_ssm::Client,
    client_instance_id: &str,
    server_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("client", ssm_client, client_instance_id, vec![
        format!("runuser -u ec2-user -- echo ec2 up > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-1", unique_id).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-2", unique_id).as_str(),
        format!("timeout 1h bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 60; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-3; exit 1)", unique_id).as_str(),
        format!("echo yum finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-3", unique_id).as_str(),
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        format!("runuser -u ec2-user -- echo git finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-4", unique_id).as_str(),
        "runuser -u ec2-user -- aws s3 cp s3://netbenchrunnerlogs/TS/request_response.json /home/ec2-user/request_response.json",
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-5", unique_id).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-6", unique_id).as_str(),
        format!("env SERVER_0={}:4433 COORD_SERVER_0={}:8080 ./scripts/netbench-test-player-as-client.sh", server_ip, server_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-7", unique_id).as_str(),
        "runuser -u ec2-user -- cd target/netbench",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/client-step-8", unique_id).as_str(),
        "shutdown -h +1",
        "exit 0"
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn execute_ssm_server(
    ssm_client: &aws_sdk_ssm::Client,
    server_instance_id: &str,
    client_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("server", ssm_client, server_instance_id, vec![
        format!("runuser -u ec2-user -- echo starting > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-1", unique_id).as_str(),
        "cd /home/ec2-user",
        "yum upgrade -y",
        format!("runuser -u ec2-user -- echo yum upgrade finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-2", unique_id).as_str(),
        format!("timeout 1h bash -c 'until yum install cmake cargo git perl openssl-devel bpftrace perf tree -y; do sleep 60; done' || (echo yum failed > /home/ec2-user/index.html; aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-3; exit 1)", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo yum install finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-3", unique_id).as_str(),
        format!("runuser -u ec2-user -- git clone --branch {} {}", STATE.branch, STATE.repo).as_str(),
        format!("runuser -u ec2-user -- echo git clone finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-4", unique_id).as_str(),
        "runuser -u ec2-user -- aws s3 cp s3://netbenchrunnerlogs/TS/request_response.json /home/ec2-user/request_response.json",
        format!("runuser -u ec2-user -- echo SCENARIO finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-5", unique_id).as_str(),
        "cd s2n-quic/netbench",
        "runuser -u ec2-user -- cargo build --release",
        "runuser -u ec2-user -- mkdir -p target/netbench",
        "runuser -u ec2-user -- cp /home/ec2-user/request_response.json target/netbench/request_response.json",
        format!("runuser -u ec2-user -- echo cargo build finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-6", unique_id).as_str(),
        format!("env COORD_CLIENT_0={}:8080 ./scripts/netbench-test-player-as-server.sh", client_ip).as_str(),
        "chown ec2-user: -R .",
        format!("runuser -u ec2-user -- echo run finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-7", unique_id).as_str(),
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        format!("runuser -u ec2-user -- echo report upload finished > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/server-step-8", unique_id).as_str(),
        "shutdown -h +1",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn orch_generate_report(s3_client: &aws_sdk_s3::Client, unique_id: &str) {
    // download results from s3 -----------------------
    let mut cmd = Command::new("aws");
    cmd.args([
        "s3",
        "sync",
        &format!("s3://{}/{}", STATE.log_bucket, unique_id),
        STATE.workspace_dir,
    ]);
    println!("{:?}", cmd);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");

    // CLI ---------------------------
    let results_path = format!("{}/results", STATE.workspace_dir);
    let report_path = format!("{}/report", STATE.workspace_dir);
    let mut cmd = Command::new("netbench-cli");
    cmd.args(["report-tree", &results_path, &report_path]);
    println!("{:?}", cmd);
    let status = cmd.status().expect("netbench-cli command failed");
    assert!(status.success(), " netbench-cli command failed");

    // upload report to s3 -----------------------
    let mut cmd = Command::new("aws");
    cmd.args([
        "s3",
        "sync",
        STATE.workspace_dir,
        &format!("s3://{}/{}", STATE.log_bucket, unique_id),
    ]);
    println!("{:?}", cmd);
    assert!(cmd.status().expect("aws sync").success(), "aws sync");

    update_report_url(s3_client, unique_id).await;

    println!("Report Finished!: Successful: true");
    println!("URL: {}/report/index.html", STATE.cf_url_with_id(unique_id));
}

async fn update_report_url(s3_client: &aws_sdk_s3::Client, unique_id: &str) {
    let body = ByteStream::new(SdkBody::from(format!(
        "<a href=\"http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html\">Final Report</a>",
        unique_id
    )));
    let key = format!("{}/finished-step-0", unique_id);
    let _ = upload_object(s3_client, STATE.log_bucket, body, &key)
        .await
        .unwrap();
}
