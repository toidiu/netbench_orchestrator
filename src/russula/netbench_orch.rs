
#[async_trait]
impl Protocol for NetbenchOrchProtocol {
    type State = NetbenchOrchState;

    async fn wait_for_coordinator(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("--- Worker listening on: {}", addr);

        let (stream, _local_addr) =
            listener
                .accept()
                .await
                .map_err(|err| RussulaError::Connect {
                    dbg: err.to_string(),
                })?;
        println!("Worker success connection: {addr}");

        Ok(stream)
    }

    async fn connect_to_worker(&self, addr: SocketAddr) -> RussulaResult<TcpStream> {
        println!("--- Coordinator: attempt to connect to worker on: {}", addr);

        let connect = TcpStream::connect(addr)
            .await
            .map_err(|err| RussulaError::Connect {
                dbg: err.to_string(),
            })?;

        Ok(connect)
    }

    async fn recv_msg(&self, stream: TcpStream) -> RussulaResult<Self::State> {
        stream.readable().await.unwrap();

        let mut buf = Vec::with_capacity(100);
        match stream.try_read_buf(&mut buf) {
            Ok(n) => {
                let msg = NetbenchOrchState::from_bytes(&buf)?;
                println!("read {} bytes: {:?}", n, &msg);
            }
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                panic!("{}", e)
            }
            Err(e) => panic!("{}", e),
        }

        // TODO
        Ok(self.state)
    }

    async fn send_msg(&self, stream: TcpStream, msg: Self::State) -> RussulaResult<()> {
        stream.writable().await.unwrap();

        stream.try_write(msg.as_bytes()).unwrap();

        Ok(())
    }

    fn state(&self) -> Self::State {
        self.state
    }
    fn peer_state(&self) -> Self::State {
        self.peer_state
    }
}
