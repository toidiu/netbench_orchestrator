// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::time::Duration;

pub fn parse_duration(s: &str) -> Result<Duration, humantime::DurationError> {
    Ok(humantime::parse_duration(s)?)
}
