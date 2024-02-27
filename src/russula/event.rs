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
use std::{fmt::Display, net::SocketAddr};
use tokio::net::TcpStream;
use tracing::{debug, info};

pub enum EventType {
    SendMsg,
    RecvMsg,
}

#[derive(Debug, Default, Clone)]
pub struct EventRecorder {
    send_msg: u64,
    recv_msg: u64,
}

impl EventRecorder {
    pub fn process(&mut self, event: EventType) {
        match event {
            EventType::SendMsg => self.send_msg += 1,
            EventType::RecvMsg => self.recv_msg += 1,
        }
    }
}

impl Display for EventRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "send_cnt: {}, recv_cnt: {}",
            self.send_msg, self.recv_msg
        );
        Ok(())
    }
}
