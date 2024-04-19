use std::{
    io::{Read, Write},
    mem::{size_of, transmute, MaybeUninit},
    net::IpAddr,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use clap::Parser;

use shared::{
    ipc::{self, ring_buffer_metadata::RingBufferMetaData},
    rdma_controller,
    ring_buffer::RingBuffer,
};
use shared_memory::ShmemConf;
use uninit::out_ref::Out;

use crate::command_line::GlobalArgs;

mod atomic_extension;
mod command_line;

pub fn main() {
    let args = GlobalArgs::parse();

    let connection_type = if args.server_addr.is_some() {
        rdma_controller::config::ConnectionType::Client {
            server_addr: IpAddr::from_str(args.server_addr.unwrap().as_str()).unwrap(),
            port: args.port.unwrap(),
            message_size: args.message_size,
        }
    } else {
        rdma_controller::config::ConnectionType::Server {
            port: args.port.unwrap(),
            message_size: args.message_size,
        }
    };

    let mut ib_resource = rdma_controller::IbResource::new();

    let config = rdma_controller::config::Config {
        dev_name: args.dev,
        connection_type: connection_type.clone(),
        gid_index: args.gid_index,
    };

    ib_resource.setup_ib(config).unwrap();

    println!("IB setup done");

    const RINGBUFFER_LEN: usize = 2048;

    let mut shmem = ShmemConf::new()
        .size(size_of::<RingBuffer<u8, RINGBUFFER_LEN>>())
        .create()
        .unwrap();

    println!("shared memory size {}", shmem.len());

    let mut mr = unsafe {
        ib_resource
            .register_memory_region(shmem.as_slice_mut())
            .unwrap()
    };

    let ring_buffer = unsafe {
        let uninit = shmem
            .as_ptr()
            .cast::<MaybeUninit<RingBuffer<u8, RINGBUFFER_LEN>>>()
            .as_mut()
            .unwrap();

        uninit.write(RingBuffer::new());

        uninit.assume_init_mut()
    };

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

    let batch_size = 1024;

    match connection_type {
        rdma_controller::config::ConnectionType::Server { message_size, .. } => loop {
            unsafe {
                let mut buffer = ring_buffer.reserve_write(message_size);

                assert_eq!(buffer.len(), message_size);

                eprintln!("buffer len {}", buffer.len());

                ib_resource
                    .post_srq_recv(2, &mut mr, Out::<'_, [u8]>::from(buffer.deref_mut()))
                    .expect("Failed to post recv");

                'outer: loop {
                    for wc in ib_resource.poll_cq() {
                        if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                            panic!(
                                "wc status {}, last error {}",
                                wc.status,
                                std::io::Error::last_os_error()
                            );
                        }

                        if wc.opcode == rdma_sys::ibv_wc_opcode::IBV_WC_RECV {
                            break 'outer;
                        }
                    }
                }
            }

            break;
        },
        rdma_controller::config::ConnectionType::Client { message_size, .. } => loop {
            if let Some(reader) = ring_buffer.read_exact(message_size) {
                assert_eq!(reader.len(), message_size);

                unsafe {
                    ib_resource
                        .post_send(2, &mut mr, reader.deref())
                        .expect("Failed to post send");
                }

                'outer: loop {
                    for wc in ib_resource.poll_cq() {
                        if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                            panic!(
                                "wc status {}, last error {}",
                                wc.status,
                                std::io::Error::last_os_error()
                            );
                        }

                        if wc.opcode == rdma_sys::ibv_wc_opcode::IBV_WC_SEND {
                            break 'outer;
                        }
                    }
                }
            }
        },
    }

    println!("Finish testing");
}
