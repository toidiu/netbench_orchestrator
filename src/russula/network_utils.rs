// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{RussulaError, RussulaResult};
use bytes::Bytes;
use tokio::net::TcpStream;

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    stream.readable().await.map_err(RussulaError::from)?;
    read_msg(stream).await
}

pub async fn send_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<usize> {
    stream.writable().await.map_err(RussulaError::from)?;
    write_msg(stream, msg).await
}

async fn write_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<usize> {
    let mut data: Vec<u8> = Vec::with_capacity((msg.len + 1).into());
    data.push(msg.len);
    data.extend(msg.data);

    stream.try_write(&data).map_err(RussulaError::from)
}

async fn read_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    let mut len_buf = [0; 1];
    stream.try_read(&mut len_buf).map_err(RussulaError::from)?;
    let len = u8::from_be_bytes(len_buf);

    let mut data = Vec::with_capacity(len.into());
    let read_bytes = stream.try_read_buf(&mut data).map_err(RussulaError::from)?;
    if read_bytes == len as usize {
        Ok(Msg::new(data.into()))
    } else {
        let data = std::str::from_utf8(&data).unwrap_or("Unable to parse bytes as str!!");
        Err(RussulaError::BadMsg {
            dbg: format!("received a malformed msg. len: {} data: {:?}", len, data),
        })
    }
}

#[derive(Debug)]
pub struct Msg {
    pub len: u8,
    pub data: Bytes,
}

impl Msg {
    pub fn new(data: Bytes) -> Msg {
        Msg {
            len: data.len() as u8,
            data,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl std::fmt::Display for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = std::str::from_utf8(&self.data).expect("expected str");
        write!(f, "Msg [ len: {} data: {} ]", self.len, data)
    }
}
