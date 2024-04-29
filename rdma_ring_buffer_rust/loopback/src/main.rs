use std::time::Duration;

mod client;
mod server;
mod spec;

fn main() {
    let spec = spec::Spec {
        port: 12345,
        message_size: 64,
        buffer_size: 2 << 15,
        duration: Duration::from_secs(3),
        batch_size: 1024,
    };

    client::connect_to_server(spec);
    server::connect_to_client(spec);
}
