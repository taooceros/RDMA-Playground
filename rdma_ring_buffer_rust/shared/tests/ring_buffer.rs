#[cfg(test)]
pub mod tests {
    #[test]
    pub fn ring_buffer_general_test() {
        use std::{sync::Arc, thread};

        use shared::ring_buffer::RingBufferConst;

        for i in 1..500 {
            thread::scope(|s| {
                let mut ring_buffer = RingBufferConst::<u64, 4096>::new();
                let ref_ring_buffer = Arc::new(ring_buffer.to_ref());
                let reader = ref_ring_buffer.clone();
                let writer = ref_ring_buffer.clone();

                const BATCH_SIZE: usize = 64;
                const ITER: usize = 1024;

                s.spawn(move || {
                    let mut count = 0;
                    for _ in 0..ITER {
                        if let Some(reader) = reader.read_exact(BATCH_SIZE) {
                            for i in 0..BATCH_SIZE {
                                assert_eq!(reader[i], count);
                                count += 1;
                            }
                        }
                    }
                });

                s.spawn(move || {
                    let mut count = 0;

                    for _ in 0..ITER {
                        if let Some(mut writer) = writer.reserve_write(BATCH_SIZE) {
                            for i in 0..BATCH_SIZE {
                                writer[i].write(count);
                                count += 1;
                            }
                        }
                    }
                });
            });
        }
    }
}
