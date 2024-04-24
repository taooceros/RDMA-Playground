use std::mem::ManuallyDrop;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use uninit::extension_traits::ManuallyDropMut;

const KB: usize = 1024;
const MB: usize = KB * KB;
const GB: usize = KB * MB;

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring buffer bench");
    for size in (4..=10).map(|i| 1 << i) {
        group.throughput(criterion::Throughput::Bytes((1 * MB) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| ring_buffer(size));
        });
    }
}

#[inline(always)]
fn ring_buffer(batch_size: usize) {
    use std::{sync::Arc, thread};

    use shared::ring_buffer::RingBuffer;

    thread::scope(|s| {
        let mut ring_buffer = RingBuffer::<u8, 4096>::new();
        let ref_ring_buffer = Arc::new(ring_buffer.to_ref());
        let reader = ref_ring_buffer.clone();
        let writer = ref_ring_buffer.clone();
        const ITER: usize = 1 * MB;

        let reader_thread = s.spawn(move || {
            let mut count = 0;
            for _ in 0..ITER {
                if let Some(reader) = reader.read_exact(batch_size) {
                    for val in reader.iter() {
                        assert_eq!(*val, count);
                        count += 1;
                    }
                }
            }
        });

        let writer_thread = s.spawn(move || {
            let mut count = 0;

            for _ in 0..ITER {
                if let Some(mut writer) = writer.reserve_write(batch_size) {
                    for val in writer.iter_mut() {
                        val.write(count);
                        count += 1;
                    }
                }
            }
        });

        reader_thread.join().unwrap();
        writer_thread.join().unwrap();
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
