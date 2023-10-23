use crate::s3_helper::*;
use crate::ssm::operation::send_command::SendCommandOutput;
use crate::state::*;
use crate::utils::*;
use aws_sdk_s3 as s3;
use aws_sdk_ssm as ssm;
use std::fs::File;
use std::io::prelude::*;
use tokio_stream::StreamExt;

pub async fn execute_ssm_client(
    ssm_client: &ssm::Client,
    client_instance_id: String,
    server_ip: &str,
    unique_id: &str,
) -> SendCommandOutput {
    send_command("client", ssm_client, &client_instance_id, vec![
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
    ssm_client: &ssm::Client,
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
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out")
}

pub async fn generate_report(
    s3_client: &s3::Client,
    ssm_client: &ssm::Client,
    server_instance_id: &str,
    unique_id: &str,
) -> bool {
    let unique_id = "";

    // mkdir -p target/netbench
    //
    // TODO get data from s3
    // format!("aws s3 sync s3://netbenchrunnerlogs/{} /home/ec2-user/s2n-quic/netbench/target/netbench", unique_id)
    download_object(s3_client, STATE.log_bucket, unique_id)
        .await
        .unwrap();

    // TODO run netbench-cli to generate report
    // "~/projects/player_netbench/target/debug/netbench-cli report-tree ./target/netbench/results ./target/netbench/report",
    //
    // TODO upload report to s3
    // format!("aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id)
    //
    // TODO update status for index.html
    // format!(r#"echo \<a href=\"http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html\"\>Final Report\</a\> > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/finished-step-0"#, unique_id, unique_id)

    let generate_report = send_command("server", ssm_client, server_instance_id, vec![
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/before-sync",
        format!("runuser -u ec2-user -- aws s3 sync s3://netbenchrunnerlogs/{} /home/ec2-user/s2n-quic/netbench/target/netbench", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync",
        "cd /home/ec2-user/s2n-quic/netbench/",
        "runuser -u ec2-user -- ./target/release/netbench-cli report-tree ./target/netbench/results ./target/netbench/report",
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-report",
        format!("runuser -u ec2-user -- aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id).as_str(),
        "runuser -u ec2-user -- tree /home/ec2-user/s2n-quic/netbench/target/netbench > /home/ec2-user/after-sync-back",
        format!(r#"runuser -u ec2-user -- echo \<a href=\"http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html\"\>Final Report\</a\> > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/finished-step-0"#, unique_id, unique_id).as_str(),
        "shutdown -h +1",
        "exit 0",
    ].into_iter().map(String::from).collect()).await.expect("Timed out");

    wait_for_ssm_results(
        "server",
        ssm_client,
        generate_report.command().unwrap().command_id().unwrap(),
    )
    .await
}

pub async fn orch_generate_report(s3_client: &s3::Client, unique_id: &str) -> bool {
    // mkdir -p target/netbench
    //
    // TODO get data from s3
    // format!("aws s3 sync s3://netbenchrunnerlogs/{} /home/ec2-user/s2n-quic/netbench/target/netbench", unique_id)
    let key = "client.json";
    let mut file = File::create(key).unwrap();

    let key = "client.json";
    let full_path = format!("{}/results/request_response/s2n-quic/{}", unique_id, key);
    let mut obj = download_object(s3_client, STATE.log_bucket, &full_path)
        .await
        .unwrap();
    // let body = obj.body;

    // let bytes = body.collect().await.map(|data| data.into_bytes());
    while let Some(bytes) = obj.body.try_next().await.unwrap() {
        let bytes = file.write(&bytes).unwrap();
    }

    // TODO run netbench-cli to generate report
    // "~/projects/player_netbench/target/debug/netbench-cli report-tree ./target/netbench/results ./target/netbench/report",
    //
    // TODO upload report to s3
    // format!("aws s3 sync /home/ec2-user/s2n-quic/netbench/target/netbench s3://netbenchrunnerlogs/{}", unique_id)
    //
    // TODO update status for index.html
    // format!(r#"echo \<a href=\"http://d2jusruq1ilhjs.cloudfront.net/{}/report/index.html\"\>Final Report\</a\> > /home/ec2-user/index.html && aws s3 cp /home/ec2-user/index.html s3://netbenchrunnerlogs/{}/finished-step-0"#, unique_id, unique_id)
    true
}
