// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    coordination_utils, dashboard,
    ec2_utils::LaunchPlan,
    error::{OrchError, OrchResult},
    report::orch_generate_report,
    ssm_utils, update_dashboard, upload_object, OrchestratorConfig,
};
use aws_sdk_s3::primitives::ByteStream;
use tracing::info;

// TODO
// - work on cluster
// - work on AZ
// W- prod account integration
// D- release dc-quic
//
// W- test with large number of hosts
//   - error with 20 hosts
//      - scenario where client doesnt finish.. presumably it is waiting for data
//      - openssl hostname mismatch error
//        - https://gist.github.com/toidiu/a0ff912e1d087608445cf876b9c860cf
//   - errors with 10 hosts
//      - s2n-quic driver wasn't installed `ssm: cargo install`
//   - enable logs on netbench
//   x- use different ports for driveres
//
// - debug dc-quic driver
// - combine client and server host launch.
//   - cleanup client if we cant provision servers and vice versa.
//   - clean up on error..
//
// # Russula/Cli
//
// # Features
// - capture driver to run as part of Scenario
// - fix graph colors in incast reports
//
// # Optimization
// - use release build instead of debug
// - experiment with uploading and downloading netbench exec
// - tar.gz private source
//   - save hash of private source
//   - get private src exec from s3
// - enum for orch steps
//   - add timing data
// - use release build instead of debug
// - experiment with uploading and downloading netbench exec
// - add logging for netbench - 2 days
//   - TRACE=stdio
// - add pcap captures
//
// # Cleanup
// - instance::poll_state should take multiple instance_ids
// - install netbench drivers from crates.io
// - cleanup dashboard
//

pub async fn run(
    unique_id: String,
    config: &OrchestratorConfig,
    aws_config: &aws_types::SdkConfig,
) -> OrchResult<()> {
    let iam_client = aws_sdk_iam::Client::new(aws_config);
    let s3_client = aws_sdk_s3::Client::new(aws_config);
    let ec2_client = aws_sdk_ec2::Client::new(&aws_config);
    let ssm_client = aws_sdk_ssm::Client::new(&aws_config);

    let scenario_file = ByteStream::from_path(&config.netbench_scenario_filepath)
        .await
        .map_err(|err| OrchError::Init {
            dbg: err.to_string(),
        })?;
    upload_object(
        &s3_client,
        config.cdk_config.netbench_runner_s3_bucket(),
        scenario_file,
        &format!("{unique_id}/{}", config.netbench_scenario_filename),
    )
    .await
    .unwrap();

    update_dashboard(
        dashboard::Step::UploadIndex,
        &s3_client,
        &unique_id,
        &config,
    )
    .await?;

    // Setup instances
    let infra = LaunchPlan::create(&unique_id, &ec2_client, &iam_client, &ssm_client, &config)
        .await
        .launch(&ec2_client, &unique_id)
        .await?;
    let client_ids: Vec<String> = infra
        .clients
        .clone()
        .into_iter()
        .map(|infra_detail| {
            let id = infra_detail.instance_id().unwrap();
            id.to_string()
        })
        .collect();
    let server_ids: Vec<String> = infra
        .servers
        .clone()
        .into_iter()
        .map(|infra_detail| {
            let id = infra_detail.instance_id().unwrap();
            id.to_string()
        })
        .collect();

    update_dashboard(
        dashboard::Step::HostsRunning(&infra.servers),
        &s3_client,
        &unique_id,
        config,
    )
    .await?;
    update_dashboard(
        dashboard::Step::HostsRunning(&infra.clients),
        &s3_client,
        &unique_id,
        config,
    )
    .await?;

    let server_drivers = vec![
        ssm_utils::s2n_quic_dc_driver::dc_quic_server_driver(&unique_id),
        ssm_utils::tcp_driver_crates::tcp_server_driver(),
        ssm_utils::s2n_quic_driver_crates::s2n_quic_server_driver(),
        ssm_utils::s2n_tls_driver::s2n_tls_server_driver(),
        // ssm_utils::native_tls_driver::native_tls_server_driver(),
    ];
    let client_drivers = vec![
        ssm_utils::s2n_quic_dc_driver::dc_quic_client_driver(&unique_id),
        ssm_utils::tcp_driver_crates::tcp_client_driver(),
        ssm_utils::s2n_quic_driver_crates::s2n_quic_client_driver(),
        ssm_utils::s2n_tls_driver::s2n_tls_client_driver(),
        // ssm_utils::native_tls_driver::native_tls_client_driver(),
    ];

    assert_eq!(server_drivers.len(), client_drivers.len());

    // configure and build
    {
        let mut build_cmds = ssm_utils::common::collect_config_cmds(
            "server",
            &ssm_client,
            server_ids.clone(),
            &config,
            &server_drivers,
            &unique_id,
            config,
        )
        .await;
        let client_build_cmds = ssm_utils::common::collect_config_cmds(
            "client",
            &ssm_client,
            client_ids.clone(),
            &config,
            &client_drivers,
            &unique_id,
            config,
        )
        .await;
        build_cmds.extend(client_build_cmds);
        ssm_utils::common::wait_complete(
            "Setup hosts: update and install dependencies",
            &ssm_client,
            build_cmds,
        )
        .await;

        info!("Host setup Successful");
    }

    let driver_pairs = client_drivers.into_iter().zip(server_drivers);
    for (client_driver, server_driver) in driver_pairs {
        info!(
            "Running server: {} and client: {}",
            server_driver.driver_name(),
            client_driver.driver_name()
        );
        println!(
            "Running Netbench with server: {} and client: {}",
            server_driver.driver_name(),
            client_driver.driver_name()
        );

        // run russula
        {
            let mut server_russula = coordination_utils::ServerNetbenchRussula::new(
                &ssm_client,
                &infra,
                server_ids.clone(),
                &config,
                &server_driver,
            )
            .await;

            let mut client_russula = coordination_utils::ClientNetbenchRussula::new(
                &ssm_client,
                &infra,
                client_ids.clone(),
                &config,
                &client_driver,
            )
            .await;

            // run client/server
            server_russula.wait_workers_running(&ssm_client).await;
            client_russula.wait_done(&ssm_client).await;
            server_russula.wait_done(&ssm_client).await;
        }

        // copy netbench results
        {
            let copy_server_netbench = ssm_utils::server::upload_netbench_data(
                &ssm_client,
                server_ids.clone(),
                &unique_id,
                &config,
                &server_driver,
            )
            .await;
            let copy_client_netbench = ssm_utils::client::upload_netbench_data(
                &ssm_client,
                client_ids.clone(),
                &unique_id,
                &config,
                &client_driver,
            )
            .await;
            let msg = format!(
                "copy netbench results to s3 for drivers: {}, {}",
                server_driver.trim_driver_name(),
                client_driver.trim_driver_name()
            );
            ssm_utils::common::wait_complete(
                &msg,
                &ssm_client,
                vec![copy_server_netbench, copy_client_netbench],
            )
            .await;
            info!("client_server netbench copy results!: Successful");
        }
    }

    // Copy results back
    orch_generate_report(&s3_client, &unique_id, &infra, config).await;

    // Cleanup
    infra
        .cleanup(&ec2_client)
        .await
        .map_err(|err| eprintln!("Failed to cleanup resources. {}", err))
        .unwrap();

    Ok(())
}
