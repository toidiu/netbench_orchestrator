// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::error::{RussulaError, RussulaResult};
use crate::russula::protocol::Protocol;
use crate::russula::NextTransitionMsg;
use crate::russula::StateApi;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

// enum NetbenchServerCoordState {
#[derive(Clone, Copy)]
struct CoordCheckPeer;
#[derive(Clone, Copy)]
struct CoordReady;
#[derive(Clone, Copy)]
struct CoordRunPeer;
#[derive(Clone, Copy)]
struct CoordKillPeer;
#[derive(Clone, Copy)]
struct CoordDone;

// enum NetbenchServerWorkerState {
#[derive(Clone, Copy)]
struct ServerWaitPeerReady;
#[derive(Clone, Copy)]
struct ServerReady;
#[derive(Clone, Copy)]
struct ServerRun;
#[derive(Clone, Copy)]
struct ServerDone;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
enum NetbenchServerStateMachine {
    AA_1((CoordCheckPeer, ServerWaitPeerReady)),
    AB_2((CoordCheckPeer, ServerReady)),
    BB_3((CoordReady, ServerReady)),
    CB_4((CoordRunPeer, ServerReady)),
    CC_5((CoordRunPeer, ServerRun)),
    DC_6((CoordKillPeer, ServerRun)),
    DD_7((CoordKillPeer, ServerDone)),
    ED_8((CoordDone, ServerDone)),
}

impl Default for NetbenchServerStateMachine {
    fn default() -> Self {
        NetbenchServerStateMachine::AA_1((CoordCheckPeer, ServerWaitPeerReady))
    }
}

#[derive(Clone, Copy, Default)]
pub struct NetbenchWorkerServerProtocol {
    state: NetbenchServerStateMachine,
}
