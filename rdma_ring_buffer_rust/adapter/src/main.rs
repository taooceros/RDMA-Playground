use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    mem::size_of_val,
    net::IpAddr,
    str::FromStr,
};

use clap::Parser;
use nix::libc::mkfifo;
use shared::{ipc::{self, ring_buffer_metadata::RingBufferMetaData}, ring_buffer::RingBuffer};
use shared_memory::ShmemConf;

use crate::command_line::GlobalArgs;

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

    assert_eq!(ib_resource.setup_ib(config).unwrap(), 0);

    let buffer = ib_resource.allocate_buffer();

    const RINGBUFFER_LEN: usize = 2048;

    buffer.write(RingBuffer::<u8, RINGBUFFER_LEN>::new());

    let mut ipc = ipc::Ipc::new("sync");

    let init_metadata = RingBufferMetaData {
        ring_buffer_len: RINGBUFFER_LEN,
        shared_memory_name: shmem.get_os_id().as_bytes().into(),
    };

    init_metadata.write_to_ipc(&mut ipc);

    loop {
        let mut buf = [0u8; 1];
        ipc.read(&mut buf).unwrap();
        if buf[0] == 1 {
            break;
        }
    }
}
