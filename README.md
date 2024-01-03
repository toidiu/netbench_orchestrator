# Netbench Orchestrator

Used to run netbench scenarios at scale.

## Goals
Often developers of transport protocols are interested in gather performance data for the protocol
they are developing. Netbench is a tool which can be used to measure this performance data.
However, in-order to get useful results its often necessary to run Netbench scenarios in the cloud
so that the results better match production systems. The goal of this project is to automate
Netbench runs in the cloud.

## Getting started

**Pre-requsites**
- Built and include [netbench](https://github.com/aws/s2n-quic/netbench) utilities (`cargo build`)
  - Include in PATH `export PATH="...path_to_netbench.../s2n-quic/netbench/target/release/:$PATH"`
  - `which netbench-cli`: TODO move to prequesite netbench checks
- AWS cli is installed
  - `which aws`: TODO move to prequesite netbench checks
- An AWS account with some infrastructure configured. TODO: provide an easy way to do this
  - Make sure AWS credentials are included in your shell environment

**Running**

```
git clone git@github.com:toidiu/netbench_orchestrator.git && cd netbench_orchestrator

# Run the orchestrator
make orch
```

## Implementation details

### Russula
Russula is a synchronization/coordination framework where a single Coordinator can be used to drive
multiple Workers. This is driven by the need to test multiple server/client incast Netbench
scenario.

At its basis an instance of Russula is composed of a pair of Coordinator/Worker Protocols. Currently
its possible to create an instance of NetbenchServer and NetbenchClient, which can be used to run
a multi server/client netbench scenario.

Since Russula is used to run Netbench testing it has the following goals:
- non-blocking: its not acceptable to block since we are trying to do performance testing
- minimal network noise: since we are trying to measure transport protocols, the coordination protocol
should add minimal traffic to the network and ideally none during the actual testing
- easily configurable: the protocol should allow for new states to allow for expanding usecases
- secure: the protocol should not accept executable code since this opens it up for code execution attack.
- easy to develop: exposes logging and introspection into the peers states to allow for easy debugging
- resilient: should be resilient to errors (network or otherwise); retrying requests when they are considered
non-fatal

#### Russula deep dive
For a detailed description
of a state machine pair, take a look at the [netbench module](src/russula/netbench.rs). A Netbench
run might look something like this on the coordinator:

```
let server_ip_list = [...];
let client_ip_list = [...];

// use ssm or something equivalent to run the Worker protocol on the Worker hosts.
// pseudo-code below
ssm.connect(server_ip_list).run("cargo run --bin russula_runner NetbenchServerWorker");
ssm.connect(client_ip_list).run("cargo run --bin russula_runner NetbenchClientWorker");

let russula_server_coord: Russula<NetbenchServerCoordinator> = Russula::new(server_ip_list);
let russula_client_coord: Russula<NetbenchServerCoordinator> = Russula::new(client_ip_list);

// confirm all the workers are ready
russula_server_coord.run_till_ready().await;
russula_client_coord.run_till_ready().await;

// start the Netbench server hosts
russula_server_coord
    .run_till_state(server::CoordState::WorkersRunning, || {})
    .await
    .unwrap();
tokio::time::sleep(Duration::from_secs(5)).await;

// start the Netbench client hosts and wait till they all finish
russula_client_coord
    .run_till_state(client::CoordState::Done, || {})
    .await
    .unwrap();

// tell all server hosts to complete/terminate since the netbench scenarios is complete
russula_server_coord
    .run_till_state(server::CoordState::Done, || {})
    .await
    .unwrap();
```
