use std::{
    fs::File,
    io::{Read, Write},
    mem::{size_of, MaybeUninit},
};

use clap::Parser;
use rand::random;
use shared::ref_ring_buffer::RefRingBuffer;
use shared_memory::ShmemConf;

use crate::command_line::{ConnectionType, GlobalArgs};

mod atomic_extension;
mod command_line;
mod communication_manager;

mod rdma_adapter;

fn main() {
    let args = GlobalArgs::parse();

    let connection_type = if args.server_addr.is_some() {
        ConnectionType::Client
    } else {
        ConnectionType::Server
    };

    let mut pipe = File::open("sync").unwrap();

    let mut buffer = [0; size_of::<usize>()];

    pipe.read_exact(&mut buffer).unwrap();

    let ring_buffer_len = usize::from_le_bytes(buffer);

    let mut buffer = vec![0; size_of::<RefRingBuffer<u32>>()];

    pipe.read_exact(&mut buffer).unwrap();

    let shmem_os_id = std::str::from_utf8(&buffer).unwrap();

    let shmem = ShmemConf::new().os_id(shmem_os_id).open().unwrap();

    pipe.write(&1u8.to_le_bytes()).unwrap();

    // let mut ring_buffer: RingBuffer<u32, 4096, _> =
    //     RingBuffer::new_alloc(&mut ib_resource);

    println!("Starting RDMA Ring Buffer Test");

    const BATCH_SIZE: usize = 64;
    const MAX_ITER: usize = 64;

    match connection_type {
        ConnectionType::Client => {
            for i in 0..MAX_ITER {
                println!("iter: {}", i);

                let mut buffer = [0; BATCH_SIZE];

                for i in 0..BATCH_SIZE {
                    buffer[i] = random();
                    println!("Write value: {}", buffer[i]);
                }

                // ring_buffer.write(&mut buffer, BATCH_SIZE);
            }
        }
        ConnectionType::Server => {
            for i in 0..MAX_ITER {
                println!("iter: {}", i);

                let buffer: [MaybeUninit<u32>; BATCH_SIZE] =
                    [MaybeUninit::uninit(); BATCH_SIZE];
                let total_count = 0;
                loop {
                    // let count = ring_buffer.read(&mut buffer, BATCH_SIZE);
                    // total_count += count;
                    // for i in 0..count {
                    //     let value = unsafe { buffer[i].assume_init() };
                    //     println!("Read value: {}", value);
                    // }

                    // if total_count >= BATCH_SIZE {
                    //     break;
                    // }
                }
            }
        }
    }
}
