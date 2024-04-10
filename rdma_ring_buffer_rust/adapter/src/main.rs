use std::net::IpAddr;

use crate::command_line::GlobalArgs;

mod command_line;
mod rdma_controller;

pub fn main() {
    let args = GlobalArgs::parse();

    let mut ib_resource = rdma_controller::IbResource::new(8192);

    let connection_type = if args.server_addr.is_some() {
        rdma_controller::config::ConnectionType::Client {
            server_addr: IpAddr::from_str(args.server_addr.unwrap().as_str()).unwrap(),
            port: args.port.unwrap(),
        }
    } else {
        rdma_controller::config::ConnectionType::Server {
            port: args.port.unwrap(),
        }
    };

    let config = rdma_controller::config::Config {
        dev_name: args.dev,
        connection_type: connection_type.clone(),
        gid_index: args.gid_index,
    };

    assert_eq!(ib_resource.setup_ib(config).unwrap(), 0);
}
