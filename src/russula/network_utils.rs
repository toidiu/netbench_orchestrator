use crate::russula::RussulaResult;
use bytes::Bytes;
use tokio::net::TcpStream;

pub async fn recv_msg(stream: &TcpStream) -> RussulaResult<Bytes> {
    stream.readable().await.unwrap();

    let mut buf = Vec::with_capacity(100);
    match stream.try_read_buf(&mut buf) {
        Ok(_n) => Ok(Bytes::from_iter(buf)),
        Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
            panic!("{}", e)
        }
        Err(e) => panic!("{}", e),
    }

    // TODO
    // Ok(self.state)
}

pub async fn send_msg(stream: &TcpStream, msg: Bytes) -> RussulaResult<()> {
    stream.writable().await.unwrap();

    stream.try_write(&msg).unwrap();

    Ok(())
}
