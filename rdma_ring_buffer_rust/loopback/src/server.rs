use std::{
    hint::spin_loop,
    mem::{size_of, transmute, MaybeUninit},
    num::NonZeroI32,
    ops::DerefMut,
    ptr::read,
    sync::atomic::{AtomicBool, AtomicUsize},
    thread,
    time::Duration,
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

pub fn connect_to_client(spec: Spec, ready: &AtomicUsize) {
    let config = rdma_controller::config::Config {
        dev_name: "rocep152s0f0".to_owned(),
        gid_index: Some(NonZeroI32::new(1).unwrap()),
        connection_type: shared::rdma_controller::config::ConnectionType::Server {
            port: spec.port,
            message_size: spec.message_size,
        },
    };

    let mut ring_buffer = RingBufferAlloc::<usize>::new(spec.buffer_size);

    let mut ref_ring_buffer = ring_buffer.to_ref();

    let (sender, receiver) = ref_ring_buffer.split();

    let stop = &AtomicBool::new(false);

    thread::scope(|s| {
        s.spawn(move || adapter(config, sender, ready, stop, spec));

        s.spawn(move || host(ready, stop, &spec, receiver));
    })
}

fn host(
    ready: &AtomicUsize,
    stop: &AtomicBool,
    spec: &spec::Spec,
    receiver: shared::ref_ring_buffer::receiver::Receiver<usize>,
) {
    let clock = Clock::new();

    ready.fetch_add(1, std::sync::atomic::Ordering::Release);

    while ready.load(std::sync::atomic::Ordering::Acquire) < 4 {
        spin_loop();
    }

    let begin = clock.now();

    let mut expected_data = 0usize;
    let mut dataflow = 0usize;

    loop {
        if clock.now() - begin > spec.duration {
            break;
        }

        if let Some(mut reader) = receiver.read_exact(spec.batch_size) {
            coz::progress!("Read data from ring buffer");

            let reader_len = reader.len();
            dataflow += reader_len;

            for data in reader.iter() {
                if *data != expected_data {
                    panic!("Data mismatch: expected {}, got {}", expected_data, *data);
                }

                expected_data = expected_data.wrapping_add(1);
            }

            reader.commit();
        }
    }

    stop.store_release(true);

    println!(
        "Server Throughput: {} MB/s",
        dataflow as f64 / 1e6 * size_of::<usize>() as f64 / (spec.duration.as_secs() as f64)
    );

    println!("Finished RDMA Ring Buffer Test");
}

fn adapter(
    config: Config,
    sender: shared::ref_ring_buffer::sender::Sender<usize>,
    ready: &AtomicUsize,
    stop: &AtomicBool,
    spec: spec::Spec,
) {
    let mut ib_resource = IbResource::new();
    ib_resource.setup_ib(config).expect("Failed to setup IB");

    let mut mr = unsafe {
        ib_resource
            .register_memory_region(transmute::<&mut [MaybeUninit<usize>], &mut [usize]>(
                sender.ring_buffer().buffer_slice(),
            ))
            .unwrap()
    };

    ready.fetch_add(1, std::sync::atomic::Ordering::Release);

    while ready.load(std::sync::atomic::Ordering::Acquire) < 4 {
        spin_loop();
    }

    let mut expected_val = 0usize;

    'outer: loop {
        unsafe {
            if let Some(mut buffer) = sender.try_reserve(spec.message_size) {
                ib_resource
                    .post_recv(2, &mut mr, Out::<'_, [usize]>::from(buffer.deref_mut()))
                    .expect("Failed to post recv");

                'polling: loop {
                    if stop.load_acquire() {
                        break 'outer;
                    }

                    for wc in ib_resource.poll_cq() {
                        if wc.status != rdma_sys::ibv_wc_status::IBV_WC_SUCCESS {
                            panic!(
                                "wc status {}, last error {}",
                                wc.status,
                                std::io::Error::last_os_error()
                            );
                        }

                        if wc.opcode == rdma_sys::ibv_wc_opcode::IBV_WC_RECV {
                            break 'polling;
                        }
                    }
                }

                buffer.commit();
            }
        }
    }
}
