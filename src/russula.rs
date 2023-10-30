use async_trait::async_trait;
use std::{collections::BTreeMap, collections::BTreeSet, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

pub struct Russula<P: Protocol> {
    role: Role<P>,
}

// TODO
// - handle coord retry on connect
// D- move connect to protocol impl

impl<P: Protocol> Russula<P> {
    pub fn new_coordinator(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = BTreeMap::new();
        addr.into_iter().for_each(|addr| {
            map.insert(addr, protocol.clone());
        });
        let role = Role::Coordinator(map);
        Self { role }
    }

    pub fn new_worker(addr: SocketAddr, protocol: P) -> Self {
        Self {
            role: Role::Worker((addr, protocol)),
        }
    }

    pub async fn connect(&self) {
        match &self.role {
            Role::Coordinator(protocol_map) => {
                for (addr, protocol) in protocol_map.iter() {
                    protocol.connect_to_worker(*addr).await;
                }
            }
            Role::Worker((addr, protocol)) => {
                protocol.wait_for_coordinator(addr).await;
            }
        }
    }

    pub async fn start(&self) {
        match &self.role {
            Role::Coordinator(_role) => todo!(),
            Role::Worker(_role) => todo!(),
        }
    }

    pub async fn kill(&self) {
        match &self.role {
            Role::Coordinator(_) => todo!(),
            Role::Worker(role) => role.1.kill(),
        }
    }

    pub async fn wait_peer_state(&self, _state: P::Message) {}
}

#[async_trait]
pub trait Protocol: Clone {
    type Message;

    // TODO replace u8 with uuid
    fn id(&self) -> u8 {
        0
    }
    fn version(&self) {}
    fn app(&self) {}

    async fn connect_to_worker(&self, _addr: SocketAddr);
    async fn wait_for_coordinator(&self, addr: &SocketAddr);

    fn start(&self) {}
    fn kill(&self) {}

    fn recv(&self) {}
    fn send(&self) {}
    fn peer_state(&self) -> Self::Message;
}

enum Role<P: Protocol> {
    Coordinator(BTreeMap<SocketAddr, P>),
    Worker((SocketAddr, P)),
}

#[derive(Clone, Copy)]
pub struct NetbenchOrchestrator {
    peer_state: NetbenchState,
}

impl NetbenchOrchestrator {
    pub fn new() -> Self {
        NetbenchOrchestrator {
            peer_state: NetbenchState::Ready,
        }
    }
}

#[async_trait]
impl Protocol for NetbenchOrchestrator {
    type Message = NetbenchState;

    async fn wait_for_coordinator(&self, addr: &SocketAddr) {
        let listener = TcpListener::bind(addr).await.unwrap();
        println!("--- Worker listening on: {}", addr);
        match listener.accept().await {
            Ok((_socket, _local_addr)) => println!("Worker success connection: {addr}"),
            Err(e) => panic!("couldn't get client: {e:?}"),
        }
    }

    async fn connect_to_worker(&self, addr: SocketAddr) {
        println!("--- Coordinator: attempt to connect to worker on: {}", addr);

        let connect = TcpStream::connect(addr);
        match connect.await {
            Ok(_) => println!("Coordinator: successfully connected to {}", addr),
            Err(_) => println!("failed to connect to worker {}", addr),
        }
    }

    fn peer_state(&self) -> Self::Message {
        self.peer_state
    }
}

#[derive(Copy, Clone)]
pub enum NetbenchState {
    Ready,
    Run,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test() {
        let w1 = SocketAddr::from_str("127.0.0.1:8991").unwrap();
        let w2 = SocketAddr::from_str("127.0.0.1:8992").unwrap();

        let test_protocol = NetbenchOrchestrator::new();
        let addr = BTreeSet::from_iter([w1, w2]);

        let w1 = tokio::spawn(async move {
            let _worker = Russula::new_worker(w1, test_protocol).connect().await;
        });
        let w2 = tokio::spawn(async move {
            let _worker = Russula::new_worker(w2, test_protocol).connect().await;
        });

        let c1 = tokio::spawn(async move {
            let _coord = Russula::new_coordinator(addr, test_protocol)
                .connect()
                .await;
        });

        tokio::join!(w1, w2, c1).0.unwrap();

        assert!(1 == 43)
    }
}
