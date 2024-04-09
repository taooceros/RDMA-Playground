use std::{mem::MaybeUninit, net::IpAddr, str::FromStr};

use clap::Parser;
use rand::random;
use rdma_controller::IbResource;
use rdma_ring_buffer::RingBuffer;

use crate::command_line::GlobalArgs;

mod atomic_extension;
mod command_line;
mod communication_manager;
mod rdma_controller;
pub mod rdma_ring_buffer;

fn main() {
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

    let mut ring_buffer: RingBuffer<u32, 4096, _> =
        rdma_ring_buffer::RingBuffer::new_alloc(&mut ib_resource);

    println!("Starting RDMA Ring Buffer Test");

    const NUMBER_MESSAGE: usize = 64;

    match connection_type {
        rdma_controller::config::ConnectionType::Client { .. } => {
            println!("Client sending data");

            let mut buffer = [0; NUMBER_MESSAGE];

            for i in 0..NUMBER_MESSAGE {
                buffer[i] = random();
            }

            ring_buffer.write(&mut buffer, NUMBER_MESSAGE);
        }
        rdma_controller::config::ConnectionType::Server { .. } => {
            println!("Server waiting for data");

            let mut buffer: [MaybeUninit<u32>; NUMBER_MESSAGE] =
                [MaybeUninit::uninit(); NUMBER_MESSAGE];
            let mut total_count = 0;
            loop {
                let count = ring_buffer.read(&mut buffer, NUMBER_MESSAGE);
                total_count += count;
                for i in 0..count {
                    let value = unsafe { buffer[i].assume_init() };
                    println!("Read value: {}", value);
                }

                if total_count >= NUMBER_MESSAGE {
                    break;
                }
            }
        }
    }
}
