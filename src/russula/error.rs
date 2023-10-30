// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

pub type RussulaResult<T, E = RussulaError> = Result<T, E>;

#[derive(Debug)]
pub enum RussulaError {
    Connect { dbg: String },
    BadMsg { dbg: String },
}

impl std::fmt::Display for RussulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RussulaError::Connect { dbg } => write!(f, "{}", dbg),
            RussulaError::BadMsg { dbg } => write!(f, "{}", dbg),
        }
    }
}

impl std::error::Error for RussulaError {}
