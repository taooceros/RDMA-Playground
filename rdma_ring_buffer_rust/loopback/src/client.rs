use std::{
    hint::spin_loop,
    mem::{size_of, transmute, MaybeUninit},
    net::{IpAddr, Ipv4Addr},
    num::NonZeroI32,
    ops::Deref,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
};

use quanta::Clock;
use shared::{
    atomic_extension::AtomicExtension,
    rdma_controller::{
        config::{self, Config},
        IbResource,
    },
    ref_ring_buffer::sender::Sender,
    ring_buffer::RingBufferAlloc,
};

use crate::spec;

pub fn connect_to_server(spec: spec::Spec, ready: &AtomicUsize) {
    let config = config::Config {
        dev_name: "rocep152s0f0".to_owned(),
        gid_index: Some(NonZeroI32::new(1).unwrap()),
        connection_type: shared::rdma_controller::config::ConnectionType::Client {
            port: spec.port,
            message_size: spec.message_size,
            server_addr: Ipv4Addr::LOCALHOST.into(),
        },
    };

    let mut ring_buffer = RingBufferAlloc::<usize>::new(spec.buffer_size);

    let mut ref_ring_buffer = ring_buffer.to_ref();

    let (sender, receiver) = ref_ring_buffer.split();

    let stop = &AtomicBool::new(false);

    thread::scope(|s| {
        s.spawn(move || adapter(config, receiver, ready, stop, spec));

        s.spawn(move || host(ready, stop, &spec, sender));
    })
}

fn host(ready: &AtomicUsize, stop: &AtomicBool, spec: &spec::Spec, sender: Sender<usize>) {
    let clock = Clock::new();

    ready.fetch_add(1, Ordering::Release);

    while ready.load_acquire() < 4 {
        spin_loop()
    }

    let begin = clock.now();
    let mut expected_data = 0usize;
    let mut dataflow = 0usize;

    'outer: loop {
        if clock.now() - begin > spec.duration {
            break 'outer;
        }

        if let Some(mut writer) = sender.try_reserve(spec.batch_size) {
            for val in writer.iter_mut() {
                val.write(expected_data);
                expected_data = expected_data.wrapping_add(1);
            }
            dataflow += spec.batch_size;

            writer.commit();
        }
    }

    stop.store_release(true);

    println!(
        "Server Throughput: {} MB/s",
        dataflow as f64 / 1e6 * size_of::<usize>() as f64 / (spec.duration.as_secs() as f64)
    );
}

fn adapter(
    config: Config,
    receiver: shared::ref_ring_buffer::receiver::Receiver<usize>,
    ready: &AtomicUsize,
    stop: &AtomicBool,
    spec: spec::Spec,
) {
    let mut ib_resource = IbResource::new();
    ib_resource.setup_ib(config).expect("Failed to setup IB");

    let mut mr = unsafe {
        ib_resource
            .register_memory_region(transmute::<&mut [MaybeUninit<usize>], &mut [usize]>(
                receiver.ring_buffer().buffer_slice(),
            ))
            .unwrap()
    };

    ready.fetch_add(1, Ordering::Release);

    while ready.load_acquire() < 4 {
        spin_loop();
    }

    'outer: loop {
        if let Some(mut reader) = receiver.read_exact(spec.message_size) {
            unsafe {
                ib_resource
                    .post_send(2, &mut mr, reader.deref(), true)
                    .expect("Failed to post send");
            }

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

                    if wc.opcode == rdma_sys::ibv_wc_opcode::IBV_WC_SEND {
                        break 'polling;
                    }
                }
            }

            reader.commit();
        }
    }
}
