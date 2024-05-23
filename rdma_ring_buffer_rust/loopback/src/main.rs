use std::{thread, time::Duration};

mod client;
mod server;
mod spec;

fn main() {
    let spec = spec::Spec {
        port: 12345,
        message_size: 2 << 13,
        buffer_size: 2 << 20,
        duration: Duration::from_secs(5),
        batch_size: 2 << 13,
    };

    let ready = &std::sync::atomic::AtomicUsize::new(0);

    thread::scope(|s| {
        s.spawn(move || client::connect_to_server(spec, ready));
        s.spawn(move || server::connect_to_client(spec, ready));
    })
}
