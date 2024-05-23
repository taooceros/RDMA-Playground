use shared::{atomic_extension::AtomicExtension, ring_buffer::RingBufferAlloc};
use std::mem::{size_of, ManuallyDrop};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uninit::extension_traits::ManuallyDropMut;

const KB: usize = 1024;
const MB: usize = KB * KB;
const GB: usize = KB * MB;

pub fn ringbuffer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring buffer bench");
    for buffer_size in (12..=14).map(|i| 1 << i) {
        for batch_size in (3..=6).map(|i| 1 << (i * 2)) {
            group.throughput(criterion::Throughput::Bytes((size_of::<u64>() * MB) as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("Buffer Size {}", buffer_size), batch_size),
                &(buffer_size, batch_size),
                |b, &(buffer_size, batch_size)| {
                    b.iter(|| ring_buffer(batch_size, buffer_size));
                },
            );
        }
    }
}

#[inline(always)]
fn ring_buffer(batch_size: usize, buffer_size: usize) {
    use std::{sync::Arc, thread};

    use shared::ring_buffer::RingBufferConst;

    let mut ring_buffer = RingBufferAlloc::new(buffer_size);
    let mut ref_ring_buffer = ring_buffer.to_ref();

    thread::scope(|s| {
        let (sender, receiver) = ref_ring_buffer.split();

        const DATA_SIZE: usize = 1 * MB;
        let iteration = DATA_SIZE / batch_size;

        let reader_thread = s.spawn(move || {
            let mut count = 0usize;
            for _ in 0..iteration {
                loop {
                    if let Some(mut reader) = receiver.read_exact(batch_size) {
                        assert_eq!(reader.len(), batch_size);

                        for val in reader.iter() {
                            assert_eq!(*val, count);
                            count = count.wrapping_add(1);
                        }

                        reader.commit();

                        break;
                    }
                }
            }
        });

        let writer_thread = s.spawn(move || {
            let mut count = 0usize;

            for _ in 0..iteration {
                loop {
                    if let Some(mut writer) = sender.try_reserve(batch_size) {
                        assert_eq!(writer.len(), batch_size);

                        for val in writer.iter_mut() {
                            val.write(count);
                            count = count.wrapping_add(1);
                        }

                        writer.commit();

                        break;
                    }
                }
            }
        });

        reader_thread.join().unwrap();
        writer_thread.join().unwrap();

        assert_eq!(
            ring_buffer.head.load_acquire(),
            ring_buffer.tail.load_acquire()
        );
    });
}

criterion_group!(benches, ringbuffer_benchmark);
criterion_main!(benches);
