use crate::russula::{RussulaError, RussulaResult};
use bytes::Bytes;
use tokio::net::TcpStream;

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    stream
        .readable()
        .await
        .map_err(|err| RussulaError::NetworkFail {
            dbg: err.to_string(),
        })?;
    read_msg(stream).await
}

pub async fn send_msg(stream: &TcpStream, msg: Bytes) -> RussulaResult<()> {
    stream
        .writable()
        .await
        .map_err(|err| RussulaError::NetworkFail {
            dbg: err.to_string(),
        })?;

    write_msg(stream, Msg::new(msg)).await
}

async fn write_msg(stream: &TcpStream, msg: Msg) -> RussulaResult<()> {
    stream
        .try_write(&msg.len.to_be_bytes())
        .map_err(|err| RussulaError::NetworkFail {
            dbg: err.to_string(),
        })?;

    stream
        .try_write(&msg.data)
        .map_err(|err| RussulaError::NetworkFail {
            dbg: err.to_string(),
        })?;

    Ok(())
}

async fn read_msg(stream: &TcpStream) -> RussulaResult<Msg> {
    let mut len_buf = [0; 2];
    stream
        .try_read(&mut len_buf)
        .map_err(|err| RussulaError::NetworkFail {
            dbg: err.to_string(),
        })?;
    let len = u16::from_be_bytes(len_buf);

    let mut data = Vec::with_capacity(len.into());
    match stream.try_read_buf(&mut data) {
        Ok(n) => {
            if n == len.into() {
                Ok(Msg::new(data.into()))
            } else {
                Err(RussulaError::BadMsg {
                    dbg: format!("received a malformed msg. len: {} data: {:?}", len, data),
                })
            }
        }
        Err(ref err) if err.kind() == tokio::io::ErrorKind::WouldBlock => {
            Err(RussulaError::NetworkBlocked {
                dbg: err.to_string(),
            })
        }
        Err(err) => Err(RussulaError::NetworkFail {
            dbg: err.to_string(),
        }),
    }
}

#[derive(Debug)]
pub struct Msg {
    len: u16,
    data: Bytes,
}

impl Msg {
    fn new(data: Bytes) -> Msg {
        Msg {
            len: data.len() as u16,
            data,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl std::fmt::Display for Msg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = std::str::from_utf8(&self.data).unwrap();
        write!(f, "Msg [ len: {} data: {} ]", self.len, data)
    }
}
