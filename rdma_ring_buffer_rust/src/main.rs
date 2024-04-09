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

    const BATCH_SIZE: usize = 64;
    const MAX_ITER: usize = 64;

    match connection_type {
        rdma_controller::config::ConnectionType::Client { .. } => {
            for i in 0..MAX_ITER {
                println!("iter: {}", i);

                let mut buffer = [0; BATCH_SIZE];

                for i in 0..BATCH_SIZE {
                    buffer[i] = random();
                    println!("Write value: {}", buffer[i]);
                }

                ring_buffer.write(&mut buffer, BATCH_SIZE);
            }
        }
        rdma_controller::config::ConnectionType::Server { .. } => {
            for i in 0..MAX_ITER {
                println!("iter: {}", i);

                let mut buffer: [MaybeUninit<u32>; BATCH_SIZE] =
                    [MaybeUninit::uninit(); BATCH_SIZE];
                let mut total_count = 0;
                loop {
                    let count = ring_buffer.read(&mut buffer, BATCH_SIZE);
                    total_count += count;
                    for i in 0..count {
                        let value = unsafe { buffer[i].assume_init() };
                        println!("Read value: {}", value);
                    }

                    if total_count >= BATCH_SIZE {
                        break;
                    }
                }
            }
        }
    }
}
