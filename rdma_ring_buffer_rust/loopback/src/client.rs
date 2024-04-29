use std::{
    hint::spin_loop,
    mem::{transmute, MaybeUninit},
    net::Ipv4Addr,
    num::NonZeroI32,
    ops::Deref,
    sync::atomic::AtomicBool,
    thread,
};

use quanta::Clock;
use shared::{
    atomic_extension::AtomicExtension,
    rdma_controller::{
        config::{self, Config},
        IbResource,
    },
    ring_buffer::RingBufferAlloc,
};

use crate::spec;

pub fn connect_to_server(spec: spec::Spec) {
    let config = config::Config {
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
    loop {
        let clock = Clock::new();

        while !(ready).load_acquire() {
            spin_loop()
        }

        let begin = clock.now();

        let mut buffer = Vec::with_capacity(spec.batch_size);
        buffer.resize(spec.batch_size, 0);
        let mut expected_data = 0usize;
        let mut dataflow = 0usize;

        'outer: loop {
            if clock.now() - begin > spec.duration {
                break 'outer;
            }

            if let Some(mut writer) = sender.reserve_write(spec.batch_size) {
                for val in writer.iter_mut() {
                    val.write(expected_data);
                    expected_data = expected_data.wrapping_add(1);
                }
            }

            dataflow += spec.batch_size;
        }

        println!(
            "Throughput: {} MB/s",
            dataflow / 1024 / 1024 / (spec.duration.as_secs() as usize)
        );
    }
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

    if let Some(reader) = receiver.read_exact(spec.batch_size) {
        for val in reader.iter() {
            expected_val = expected_val.wrapping_add(1);
        }

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
    }
}
