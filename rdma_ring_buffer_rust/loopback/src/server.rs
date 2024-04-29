use std::num::NonZeroI32;

use shared::rdma_controller::{config::Config, IbResource};

pub fn connect_to_client(port: u16, message_size: usize) {
    let config = Config {
        dev_name: "rocep152s0f0".to_owned(),
        gid_index: Some(NonZeroI32::new(1).unwrap()),
        connection_type: shared::rdma_controller::config::ConnectionType::Server {
            port,
            message_size,
        },
    };

    let mut ib = IbResource::new();

    ib.setup_ib(config).expect("Cannot Setup Infinite Bandwidth");
}
