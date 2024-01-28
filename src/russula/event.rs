// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{
    error::RussulaError,
    network_utils,
    network_utils::Msg,
    states::{StateApi, TransitionStep},
    RussulaResult,
};
use async_trait::async_trait;
use bytes::Bytes;
use core::{fmt::Debug, task::Poll, time::Duration};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

pub enum EventType {
    SendMsg(Msg),
    RecvMsg(Msg),
}

#[derive(Debug, Default, Clone)]
pub struct EventRecorder {
    send_msg: u64,
    recv_msg: u64,
}

impl EventRecorder {
    pub fn process(&mut self, event: EventType) {
        match event {
            EventType::SendMsg(_) => self.send_msg += 1,
            EventType::RecvMsg(_) => self.recv_msg += 1,
        }
    }
}
