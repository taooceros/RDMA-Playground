use std::{thread, time::Duration};

mod client;
mod server;
mod spec;

fn main() {
    let spec = spec::Spec {
        port: 12345,
        message_size: 2 << 16,
        buffer_size: 2 << 24,
        duration: Duration::from_secs(5),
        batch_size: 64,
    };

    thread::scope(|s| {
        s.spawn(move || client::connect_to_server(spec));
        s.spawn(move || server::connect_to_client(spec));
    })
}
