use std::marker::PhantomData;

struct Russula<P: Protocol> {
    version: u16,
    app: String,
    role: Role,
    id: String, // negotiated in connect
    p: P,
}

impl<P: Protocol> Russula<P> {
    fn connect(&self, ip: &str, port: u16) {
        self.p.start()
    }

    fn curr_state(&self) -> P::Message {
        self.p.curr_state()
    }

    fn start() {}
    fn stop() {}
}

pub trait Protocol {
    type Message;

    fn start(&self) {}
    fn stop(&self) {}
    fn recv(&self) {}
    fn send(&self) {}
    fn curr_state(&self) -> Self::Message;
}

enum Role {
    Coordinator,
    Worker,
}

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
