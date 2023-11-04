use crate::russula::{RussulaError, RussulaResult};
use bytes::Bytes;
use tokio::net::TcpStream;

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Bytes> {
    stream.readable().await.unwrap();

    let mut buf = Vec::with_capacity(100);
    match stream.try_read_buf(&mut buf) {
        Ok(_n) => Ok(Bytes::from_iter(buf)),
        Err(ref err) if err.kind() == tokio::io::ErrorKind::WouldBlock => {
            Err(RussulaError::NetworkBlocked {
                dbg: err.to_string(),
            })
        }
        Err(err) => Err(RussulaError::NetworkFail {
            dbg: err.to_string(),
        }),
    }

    // TODO
    // Ok(self.state)
}

pub async fn send_msg(stream: &TcpStream, msg: Bytes) -> RussulaResult<()> {
    stream.writable().await.unwrap();

    stream.try_write(&msg).unwrap();

    Ok(())
}
