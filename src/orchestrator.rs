// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    coordination_utils, dashboard,
    ec2_utils::LaunchPlan,
    error::{OrchError, OrchResult},
    report::orch_generate_report,
    ssm_utils, update_dashboard, upload_object, Args, OrchestratorScenario, STATE,
};
use aws_sdk_s3::primitives::ByteStream;
use aws_types::region::Region;
use tracing::info;

// TODO
// D- categorize drive source (source, crates, github)
// D- run two drivers as part of single run
// - add tls drivers
//
// # Russula/Cli
//
// # Features
// - capture driver to run as part of Scenario
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
//
// # Cleanup
// - instance::poll_state should take multiple instance_ids
// - install netbench drivers from crates.io
// - cleanup dashboard
//

pub async fn run(
    unique_id: String,
    _args: Args,
    scenario: OrchestratorScenario,
    aws_config: &aws_types::SdkConfig,
) -> OrchResult<()> {
    let iam_client = aws_sdk_iam::Client::new(aws_config);
    let s3_client = aws_sdk_s3::Client::new(aws_config);
    let orch_provider_vpc = Region::new(STATE.vpc_region);
    let shared_config_vpc = aws_config::from_env()
        .region(orch_provider_vpc)
        .load()
        .await;
    let ec2_client = aws_sdk_ec2::Client::new(&shared_config_vpc);
    let ssm_client = aws_sdk_ssm::Client::new(&shared_config_vpc);

    let scenario_file = ByteStream::from_path(&scenario.netbench_scenario_filepath)
        .await
        .map_err(|err| OrchError::Init {
            dbg: err.to_string(),
        })?;
    upload_object(
        &s3_client,
        STATE.s3_log_bucket,
        scenario_file,
        &format!("{unique_id}/{}", scenario.netbench_scenario_filename),
    )
    .await
    .unwrap();

    update_dashboard(dashboard::Step::UploadIndex, &s3_client, &unique_id).await?;

    // Setup instances
    let infra = LaunchPlan::create(&unique_id, &ec2_client, &iam_client, &ssm_client, &scenario)
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
        dashboard::Step::ServerHostsRunning(&infra.servers),
        &s3_client,
        &unique_id,
    )
    .await?;
    update_dashboard(
        dashboard::Step::ServerHostsRunning(&infra.clients),
        &s3_client,
        &unique_id,
    )
    .await?;

    // custom driver
    // let dc_quic_server_driver = ssm_utils::dc_quic_server_driver(&unique_id, &scenario);
    // let dc_quic_client_driver = ssm_utils::dc_quic_client_driver(&unique_id, &scenario);
    // let quic_server_driver = ssm_utils::quic_server_driver(&unique_id, &scenario);
    // let quic_client_driver = ssm_utils::quic_client_driver(&unique_id, &scenario);
    // let tcp_server_driver = ssm_utils::tcp_server_driver(&unique_id, &scenario);
    // let tcp_client_driver = ssm_utils::tcp_client_driver(&unique_id, &scenario);
    let server_drivers = vec![
        // ssm_utils::dc_quic_server_driver(&unique_id, &scenario),
        ssm_utils::quic_server_driver(&unique_id, &scenario),
        ssm_utils::tcp_server_driver(&unique_id, &scenario),
    ];
    let client_drivers = vec![
        // ssm_utils::dc_quic_client_driver(&unique_id, &scenario),
        ssm_utils::quic_client_driver(&unique_id, &scenario),
        ssm_utils::tcp_client_driver(&unique_id, &scenario),
    ];

    // configure and build
    {
        let mut build_cmds = ssm_utils::common::collect_config_cmds(
            "server",
            &ssm_client,
            server_ids.clone(),
            &server_drivers,
            &unique_id,
        )
        .await;
        let client_build_cmds = ssm_utils::common::collect_config_cmds(
            "client",
            &ssm_client,
            client_ids.clone(),
            &client_drivers,
            &unique_id,
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
        // run russula
        {
            let mut server_russula = coordination_utils::ServerNetbenchRussula::new(
                &ssm_client,
                &infra,
                server_ids.clone(),
                &scenario,
                &server_driver,
            )
            .await;

            let mut client_russula = coordination_utils::ClientNetbenchRussula::new(
                &ssm_client,
                &infra,
                client_ids.clone(),
                &scenario,
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
                &scenario,
                &server_driver,
            )
            .await;
            let copy_client_netbench = ssm_utils::client::upload_netbench_data(
                &ssm_client,
                client_ids.clone(),
                &unique_id,
                &scenario,
                &client_driver,
            )
            .await;
            ssm_utils::common::wait_complete(
                "client_server_netbench_copy_results",
                &ssm_client,
                vec![copy_server_netbench, copy_client_netbench],
            )
            .await;
            info!("client_server netbench copy results!: Successful");
        }
    }

    // Copy results back
    orch_generate_report(&s3_client, &unique_id).await;

    // Cleanup
    infra
        .cleanup(&ec2_client)
        .await
        .map_err(|err| eprintln!("Failed to cleanup resources. {}", err))
        .unwrap();

    Ok(())
}
