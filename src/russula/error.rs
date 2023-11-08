// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

pub type RussulaResult<T, E = RussulaError> = Result<T, E>;

#[derive(Debug)]
pub enum RussulaError {
    NetworkFail { dbg: String },
    NetworkBlocked { dbg: String },
    BadMsg { dbg: String },
}

impl std::fmt::Display for RussulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RussulaError::NetworkFail { dbg } => write!(f, "NetworkFail {}", dbg),
            RussulaError::NetworkBlocked { dbg } => write!(f, "NetworkBlocked {}", dbg),
            RussulaError::BadMsg { dbg } => write!(f, "BadMsg {}", dbg),
        }
    }
}

impl std::error::Error for RussulaError {}

impl RussulaError {
    pub fn is_fatal(&self) -> bool {
        match self {
            RussulaError::NetworkBlocked { dbg: _ } => false,
            RussulaError::NetworkFail { dbg: _ } | RussulaError::BadMsg { dbg: _ } => true,
        }
    }
}
