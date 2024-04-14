use std::{
    fs::File,
    io::{Read, Write},
    mem::{align_of, size_of, MaybeUninit},
    slice,
    sync::atomic::AtomicUsize,
};

use clap::Parser;
use rand::random;
use shared::{
    ipc::{ring_buffer_metadata::RingBufferMetaData, Ipc},
    ref_ring_buffer::RefRingBuffer,
};
use shared_memory::ShmemConf;

use crate::command_line::{ConnectionType, GlobalArgs};

mod atomic_extension;
mod command_line;
mod communication_manager;

fn main() {
    let args = GlobalArgs::parse();

    let connection_type = if args.server_addr.is_some() {
        ConnectionType::Client
    } else {
        ConnectionType::Server
    };

    println!("Start Opening IPC");

    let mut ipc = Ipc::open("sync");

    println!("IPC Opened");

    let metadata = RingBufferMetaData::read_from_ipc(&mut ipc);

    println!("Ring Buffer Metadata: {:?}", metadata);

    let shmem_os_id = std::str::from_utf8(&metadata.shared_memory_name).unwrap();

    let shmem = ShmemConf::new().os_id(shmem_os_id).open().unwrap();

    println!("Shared Memory ID: {}", shmem.get_os_id());

    let shmem_ptr = shmem.as_ptr();

    let head_ref = unsafe { AtomicUsize::from_ptr(shmem_ptr.cast()) };
    let tail_ref = unsafe { AtomicUsize::from_ptr(shmem_ptr.add(size_of::<usize>()).cast()) };

    let mut ring_buffer = RefRingBuffer::<u8>::from_raw_parts(head_ref, tail_ref, unsafe {
        slice::from_raw_parts_mut(
            shmem_ptr.add(size_of::<usize>() * 2).cast(),
            metadata.ring_buffer_len,
        )
    });

    println!("Starting RDMA Ring Buffer Test");

    const BATCH_SIZE: usize = 64;
    const MAX_ITER: usize = 64;

    const DATA_SIZE: usize = BATCH_SIZE * MAX_ITER;

    match connection_type {
        ConnectionType::Client => {
            for i in 0..MAX_ITER {

                let mut buffer = [0; BATCH_SIZE];

                for i in 0..BATCH_SIZE {
                    buffer[i] = random();
                    println!("Write value: {}", buffer[i]);
                }

                ring_buffer.write(&mut buffer);
            }
        }
        ConnectionType::Server => {
            let mut readed_data = 0;

            for i in 0.. {
                let data = ring_buffer.read();
                readed_data += data.len();
                for item in data {
                    println!("Read value: {}", item);
                }

                if readed_data >= DATA_SIZE {
                    break;
                }
            }
        }
    }
}
