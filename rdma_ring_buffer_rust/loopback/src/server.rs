use std::{
    hint::spin_loop, mem::{transmute, MaybeUninit}, num::NonZeroI32, ops::DerefMut, ptr::read, sync::atomic::AtomicBool, thread, time::Duration
};

use quanta::Clock;
use shared::{
    atomic_extension::AtomicExtension,
    rdma_controller::{
        self,
        config::{self, Config},
        IbResource,
    },
    ring_buffer::{self, RingBufferAlloc},
};
use uninit::out_ref::Out;

use crate::spec::{self, Spec};

pub fn connect_to_client(spec: Spec) {
    let config = rdma_controller::config::Config {
        dev_name: "rocep152s0f0".to_owned(),
        gid_index: Some(NonZeroI32::new(1).unwrap()),
        connection_type: shared::rdma_controller::config::ConnectionType::Server {
            port: spec.port,
            message_size: spec.message_size,
        },
    };

    let mut ring_buffer = RingBufferAlloc::<usize>::new(spec.buffer_size);

    let ref_ring_buffer = ring_buffer.to_ref();

    let receiver = ref_ring_buffer.clone();
    let sender = ref_ring_buffer.clone();

    let ready = &AtomicBool::new(false);

    thread::scope(|s| {
        s.spawn(move || adapter(config, receiver, ready, spec));

        s.spawn(move || host(ready, &spec, sender));
    })
}

fn host(
    ready: &AtomicBool,
    spec: &spec::Spec,
    sender: shared::ref_ring_buffer::RefRingBuffer<usize>,
) {
    let clock = Clock::new();

    while !ready.load_acquire() {
        spin_loop();
    }

    let begin = clock.now();

    let mut expected_data = 0usize;
    let mut dataflow = 0usize;

    loop {
        if clock.now() - begin > spec.duration {
            break;
        }

        if let Some(reader) = sender.read_exact(spec.batch_size) {
            let reader_len = reader.len();
            dataflow += reader_len;

            for data in reader.iter() {
                if *data != expected_data {
                    eprintln!("Reader {:?}", reader);
                    panic!("Data mismatch: expected {}, got {}", expected_data, *data);
                }

                expected_data = expected_data.wrapping_add(1);
            }
        }
    }

    println!(
        "Throughput: {} MB/s",
        dataflow as f64 / 1024.0 / 1024.0 / (spec.duration.as_secs() as f64)
    );

    println!("Finished RDMA Ring Buffer Test");
}

fn adapter(
    config: Config,
    receiver: shared::ref_ring_buffer::RefRingBuffer<usize>,
    ready: &AtomicBool,
    spec: spec::Spec,
) {
    let mut ib_resource = IbResource::new();
    ib_resource.setup_ib(config).expect("Failed to setup IB");

    let mut mr = unsafe {
        ib_resource
            .register_memory_region(transmute::<&mut [MaybeUninit<usize>], &mut [usize]>(
                receiver.buffer_slice(),
            ))
            .unwrap()
    };

    ready.store_release(true);
    let mut expected_val = 0usize;

    loop {
        unsafe {
            if let Some(mut buffer) = receiver.reserve_write(spec.batch_size) {

                ib_resource
                    .post_recv(2, &mut mr, Out::<'_, [usize]>::from(buffer.deref_mut()))
                    .expect("Failed to post recv");

                'outer: loop {
                    for wc in ib_resource.poll_cq() {
                        if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                            eprintln!("Buffer Address: {:?}", buffer.as_ptr() as *const u64);
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

                for val in 0..buffer.len() {
                    assert_eq!(buffer[val].assume_init(), expected_val);
                    expected_val = expected_val.wrapping_add(1);
                }
            }
        }
    }
}
