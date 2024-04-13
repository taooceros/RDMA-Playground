use std::{
    io::{Read, Write},
    mem::transmute,
    net::IpAddr,
    str::FromStr,
};

use clap::Parser;

use shared::{
    ipc::{self, ring_buffer_metadata::RingBufferMetaData},
    ring_buffer::RingBuffer,
};
use shared_memory::ShmemConf;

use crate::{command_line::GlobalArgs, rdma_controller::IbResource};

mod atomic_extension;
mod command_line;
mod rdma_controller;

pub fn main() {
    let args = GlobalArgs::parse();

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

    let shmem = ShmemConf::new().size(8192).create().unwrap();

    let mut ib_resource = rdma_controller::IbResource::new_with_buffer(shmem.as_ptr(), shmem.len());

    let config = rdma_controller::config::Config {
        dev_name: args.dev,
        connection_type: connection_type.clone(),
        gid_index: args.gid_index,
    };
    const RINGBUFFER_LEN: usize = 2048;

    let ring_buffer = unsafe { ib_resource.allocate_buffer().as_mut().unwrap() };

    ring_buffer.write(RingBuffer::<u8, RINGBUFFER_LEN>::new());

    let ring_buffer = unsafe { ring_buffer.assume_init_mut() };

    println!("RingBuffer: {:p}", ring_buffer);
    println!("RingBuffer: {:p}", &ring_buffer.head);
    println!("RingBuffer: {:p}", &ring_buffer.tail);
    println!("RingBuffer: {:p}", &ring_buffer.buffer);

    let mut ring_buffer = ring_buffer.to_ref();

    println!("Creating IPC");

    let mut ipc = ipc::Ipc::create("sync");

    println!("IPC created");

    let init_metadata = RingBufferMetaData {
        ring_buffer_len: RINGBUFFER_LEN,
        shared_memory_name: shmem.get_os_id().as_bytes().into(),
    };

    println!("Metadata: {:?}", init_metadata);

    init_metadata.write_to_ipc(&mut ipc);

    assert_eq!(ib_resource.setup_ib(config).unwrap(), 0);

    match connection_type {
        rdma_controller::config::ConnectionType::Server { .. } => loop {
            let reader = ring_buffer.read();

            if reader.len() > 0 {
                ib_resource.send_message(reader.as_slice());
            }
        },
        rdma_controller::config::ConnectionType::Client { .. } => loop {
            let messages: &[u8] = ib_resource.recv_message();

            if messages.len() > 0 {
                ring_buffer.write(messages);
            }
        },
    }
}
