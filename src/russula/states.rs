// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{error::RussulaError, network_utils::Msg};
use crate::russula::{network_utils, RussulaResult};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll, time::Duration};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

#[derive(Debug)]
pub enum TransitionStep {
    // State machine is responsible for moving to the next state
    SelfDriven,
    // Wait for user input before moving to the next state
    UserDriven,
    // Wait for a peer msg before moving to the next state
    AwaitNext(Bytes),
    // Final step in the state machine with no next transitions
    Finished,
}

#[async_trait]
pub trait StateApi: Send + Sync + Clone + Debug + Serialize + for<'a> Deserialize<'a> {
    fn name_prefix(&self) -> String;

    fn name(&self, stream: &TcpStream) -> String {
        self.name_prefix().to_string()
    }

    fn transition_step(&self) -> TransitionStep;
    fn next_state(&self) -> Self;

    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }

    fn as_bytes(&self) -> Bytes {
        serde_json::to_string(self).unwrap().into()
    }

    fn from_msg(msg: Msg) -> RussulaResult<Self> {
        let msg_str = std::str::from_utf8(&msg.data).map_err(|_err| RussulaError::BadMsg {
            dbg: format!(
                "received a malformed msg. len: {} data: {:?}",
                msg.len, msg.data
            ),
        })?;

        serde_json::from_str(msg_str).map_err(|_err| RussulaError::BadMsg {
            dbg: format!(
                "received a malformed msg. len: {} data: {:?}",
                msg.len, msg.data
            ),
        })
    }
}
