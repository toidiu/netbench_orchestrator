use std::{collections::BTreeMap, collections::BTreeSet, net::SocketAddr};

struct Russula<P: Protocol> {
    role: Role<P>,
}

impl<P: Protocol> Russula<P> {
    fn new_coordinator(addr: BTreeSet<SocketAddr>, protocol: P) -> Self {
        let mut map = BTreeMap::new();
        addr.into_iter().for_each(|addr| {
            map.insert(addr, protocol.clone());
        });
        let role = Role::Coordinator(map);
        Self { role }
    }

    fn new_worker(protocol: P) -> Self {
        Self {
            role: Role::Worker(protocol),
        }
    }

    fn start() {}
    fn stop() {}
}

pub trait Protocol: Clone {
    type Message;

    // TODO replace u8 with uuid
    fn id(&self) -> u8 {
        0
    }
    fn version(&self) {}
    fn app(&self) {}

    fn start(&self) {}
    fn stop(&self) {}
    fn recv(&self) {}
    fn send(&self) {}
    fn curr_state(&self) -> Self::Message;
}

enum Role<P: Protocol> {
    Coordinator(BTreeMap<SocketAddr, P>),
    Worker(P),
}

#[derive(Clone)]
struct NetbenchOrchestrator {
    state: NetbenchState,
}

impl Protocol for NetbenchOrchestrator {
    type Message = NetbenchState;

    fn curr_state(&self) -> Self::Message {
        self.state
    }
}

#[derive(Copy, Clone)]
enum NetbenchState {}
