use std::{
    fs::File,
    io::{Read, Write},
    mem::{align_of, size_of, MaybeUninit},
    slice,
    sync::atomic::AtomicUsize,
    time::Duration,
};

use clap::Parser;
use quanta::Clock;
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

    let connection_type = args.command;

    let batch_size = args.batch_size.get();
    let duration = Duration::from_secs(args.duration.get());

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
    let mut buffer = vec![0; batch_size];

    let clock = Clock::new();

    let begin = clock.now();
    let mut dataflow = 0;

    let mut expected_data: u8 = 0;

    match connection_type {
        ConnectionType::Server => loop {
            let reader = ring_buffer.read();
            dataflow += reader.len();

            for data in reader {
                if *data != expected_data {
                    println!(
                        "Data Mismatch: Expected: {}, Actual: {}; at dataflow: {}",
                        expected_data, data, dataflow
                    );
                }
            }
        },
        ConnectionType::Client => loop {
            if clock.now() - begin > duration {
                break;
            }

            for i in 0..batch_size {
                buffer[i] = expected_data;
                expected_data = expected_data.wrapping_add(1);
                println!("Write value: {}", buffer[i]);
            }

            ring_buffer.write(&mut buffer);

            dataflow += batch_size;
        },
    }

    println!("Readed Data: {}", dataflow);
    println!(
        "Throughput: {} GB/s",
        dataflow as f64 / duration.as_secs_f64() / 1024.0 / 1024.0
    );

    println!("Finished RDMA Ring Buffer Test");
}
