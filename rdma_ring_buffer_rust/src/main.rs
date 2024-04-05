use std::{net::IpAddr, str::FromStr};

use clap::Parser;
use rdma_controller::IbResource;

use crate::command_line::GlobalArgs;

mod atomic_extension;
mod command_line;
mod rdma_controller;
pub mod rdma_ring_buffer;

fn main() {
    let args = GlobalArgs::parse();

    let mut ib_resource: IbResource<'static, 8192> = rdma_controller::IbResource::new();

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
        connection_type,
    };

    ib_resource.setup_ib(config).unwrap();
}
