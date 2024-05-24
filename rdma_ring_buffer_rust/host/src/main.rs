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

    let metadata = RingBufferMetaData::read_from(&mut ipc);

    println!("Ring Buffer Metadata: {:?}", metadata);

    let shmem_os_id =
        std::str::from_utf8(&metadata.shared_memory_name[..metadata.shared_memory_name_len])
            .unwrap();

    let shmem = ShmemConf::new().os_id(shmem_os_id).open().unwrap();

    println!("Shared Memory ID: {}", shmem.get_os_id());

    let shmem_ptr = shmem.as_ptr();

    let head_ref = unsafe {
        AtomicUsize::from_ptr(
            shmem_ptr
                .byte_add(metadata.head_offset.try_into().unwrap())
                .cast(),
        )
    };
    let tail_ref = unsafe {
        AtomicUsize::from_ptr(
            shmem_ptr
                .byte_add(metadata.tail_offset.try_into().unwrap())
                .cast(),
        )
    };

    let mut ring_buffer = RefRingBuffer::<u64>::from_raw_parts(head_ref, tail_ref, unsafe {
        slice::from_raw_parts_mut(
            shmem_ptr
                .byte_add(metadata.buffer_offset.try_into().unwrap())
                .cast(),
            metadata.ring_buffer_len,
        )
    });

    let (sender, receiver) = ring_buffer.split();

    println!("Starting RDMA Ring Buffer Test");
    let mut buffer = vec![0; batch_size];

    let clock = Clock::new();

    let begin = clock.now();
    let mut dataflow = 0;

    let mut expected_data: u64 = 0;

    match connection_type {
        ConnectionType::Server => loop {
            if clock.now() - begin > duration {
                break;
            }

            let mut chunk = receiver.read();

            let reader_len = chunk.len();
            dataflow += reader_len;

            for data in chunk.iter() {
                if *data != expected_data {
                    eprintln!("Reader {:?}", chunk);
                    panic!("Data mismatch: expected {}, got {}", expected_data, *data);
                }

                expected_data = expected_data.wrapping_add(1);
            }

            chunk.commit();
        },
        ConnectionType::Client => 'outer: loop {
            for val in buffer.iter_mut() {
                *val = expected_data;
                expected_data = expected_data.wrapping_add(1);
                // println!("Write value: {}", buffer[i]);
            }

            loop {
                if clock.now() - begin > duration {
                    break 'outer;
                }

                let write_len = sender.write(&mut buffer);
                if write_len == batch_size {
                    break;
                }
            }

            dataflow += batch_size;
        },
    }

    println!("Process Data: {}", dataflow);
    println!(
        "Throughput: {} MB/s",
        (dataflow * size_of::<u64>()) as f64 / duration.as_secs_f64() / 1024.0 / 1024.0
    );

    ipc.write(&[1]).unwrap();

    println!("Finished RDMA Ring Buffer Test");
}
