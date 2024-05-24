use core::panic;
use std::{
    io::{Read, Write},
    mem::{offset_of, size_of, transmute, MaybeUninit},
    net::IpAddr,
    ops::{Deref, DerefMut},
    process::exit,
    str::FromStr,
    thread,
};

use clap::Parser;

use shared::{
    ipc::{self, ring_buffer_metadata::RingBufferMetaData},
    rdma_controller,
    ring_buffer::RingBufferConst,
};
use shared_memory::ShmemConf;
use uninit::out_ref::Out;

use crate::{atomic_extension::AtomicExtension, command_line::GlobalArgs};

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

    const RINGBUFFER_LEN: usize = 1 << 20;

    let mut shmem = ShmemConf::new()
        .size(size_of::<RingBufferConst<u64, RINGBUFFER_LEN>>())
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
            .cast::<MaybeUninit<RingBufferConst<u64, RINGBUFFER_LEN>>>()
            .as_mut()
            .unwrap();

        uninit.write(RingBufferConst::new());

        uninit.assume_init_mut()
    };

    println!("RingBuffer: {:p}", ring_buffer);
    println!("RingBuffer: {:p}", &ring_buffer.head);
    println!("RingBuffer: {:p}", &ring_buffer.tail);
    println!("RingBuffer: {:p}", &ring_buffer.buffer);

    let mut ring_buffer = ring_buffer.to_ref();

    let (sender, receiver) = ring_buffer.split();

    println!("Creating IPC");

    let mut ipc = ipc::Ipc::create("sync");

    println!("IPC created");

    let mut name_buffer = [0u8; 32];
    let name = shmem.get_os_id();
    let name = name.as_bytes();
    name_buffer[..name.len()].copy_from_slice(name);

    let init_metadata = RingBufferMetaData {
        head_offset: offset_of!(RingBufferConst<u64, RINGBUFFER_LEN>, head),
        tail_offset: offset_of!(RingBufferConst<u64, RINGBUFFER_LEN>, tail),
        buffer_offset: offset_of!(RingBufferConst<u64, RINGBUFFER_LEN>, buffer),
        ring_buffer_len: RINGBUFFER_LEN,
        shared_memory_name_len: name.len(),
        shared_memory_name: name_buffer,
    };

    println!("Metadata: {:?}", init_metadata);

    init_metadata.write_to(&mut ipc);

    let _ipc_thread = thread::spawn(move || {
        let mut buf = vec![0];
        ipc.read_to_end(&mut buf).unwrap();
        exit(0);
    });

    let mut expected_val: u64 = 0;

    match connection_type {
        rdma_controller::config::ConnectionType::Server { message_size, .. } => loop {
            unsafe {
                if let Some(mut writer) = sender.try_reserve(message_size) {
                    assert_eq!(writer.len(), message_size);

                    ib_resource
                        .post_recv(2, &mut mr, Out::<'_, [u64]>::from(writer.deref_mut()))
                        .expect("Failed to post recv");

                    'outer: loop {
                        for wc in ib_resource.poll_cq() {
                            if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                                eprintln!("Buffer Address: {:?}", writer.as_ptr() as *const u64);
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

                    // for val in 0..writer.len() {
                    //     if writer[val].assume_init() != expected_val {
                    //         eprintln!(
                    //             "Expected: {}, Got: {}",
                    //             expected_val,
                    //             writer[val].assume_init()
                    //         );
                    //         eprintln!(
                    //             "Buffer: {:?}",
                    //             transmute::<&mut [MaybeUninit<u64>], &mut [u64]>(
                    //                 writer.deref_mut()
                    //             )
                    //         );
                    //         panic!("");
                    //     }
                    //     expected_val = expected_val.wrapping_add(1);
                    // }

                    writer.commit();
                }
            }
        },
        rdma_controller::config::ConnectionType::Client { message_size, .. } => loop {
            if let Some(mut reader) = receiver.read_exact(message_size) {
                assert_eq!(reader.len(), message_size);

                // for val in reader.iter() {
                //     if *val != expected_val {
                //         eprintln!("Buffer: {:?}", reader);
                //         panic!("");
                //     }
                //     expected_val = expected_val.wrapping_add(1);
                // }

                unsafe {
                    ib_resource
                        .post_send(2, &mut mr, reader.deref(), true)
                        .expect("Failed to post send");
                }

                'polling: loop {
                    for wc in ib_resource.poll_cq() {
                        println!("Received work completion: {:?}", wc);
                        if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                            panic!(
                                "wc status {}, last error {}",
                                wc.status,
                                std::io::Error::last_os_error()
                            );
                        }

                        if wc.opcode == rdma_sys::ibv_wc_opcode::IBV_WC_SEND {
                            break 'polling;
                        }
                    }
                }

                reader.commit();
            }
        },
    }
}
