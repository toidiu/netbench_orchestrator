use std::marker::PhantomData;

struct Russula<P: Protocol> {
    version: u16,
    app: String,
    role: Role,
    id: String, // negotiated in connect
    _p: PhantomData<P>,
}

impl<P: Protocol> Russula<P> {
    fn connect(&self, ip: &str, port: u16) {
        todo!()
    }

    fn state(&self) -> P::States {
        todo!()
    }

    fn start() {}

    fn stop() {}
}

pub trait Protocol {
    type States;

    fn recv();
    fn send();
}

enum Role {
    Coordinator,
    Worker,
}
