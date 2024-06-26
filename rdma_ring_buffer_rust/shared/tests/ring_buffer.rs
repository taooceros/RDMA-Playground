#[cfg(test)]
pub mod tests {
    #[test]
    pub fn ring_buffer_general_test() {
        use std::{sync::Arc, thread};

        use shared::ring_buffer::RingBufferConst;

        let mut ring_buffer = RingBufferConst::<u64, 8192>::new();
        let mut ref_ring_buffer = ring_buffer.to_ref();
        thread::scope(|s| {
            let (sender, receiver) = ref_ring_buffer.split();

            const BATCH_SIZE: usize = 64;
            const ITER: usize = 1024 * 512;

            let raeder_thread = s.spawn(move || {
                let mut count = 0;
                for _ in 0..ITER {
                    if let Some(reader) = receiver.read_exact(BATCH_SIZE) {
                        for i in 0..BATCH_SIZE {
                            assert_eq!(reader[i], count);
                            count += 1;
                        }
                    }
                }
            });

            let writer_thread = s.spawn(move || {
                let mut count = 0;

                for _ in 0..ITER {
                    if let Some(mut writer) = sender.try_reserve(BATCH_SIZE) {
                        for i in 0..BATCH_SIZE {
                            writer[i].write(count);
                            count += 1;
                        }
                    }
                }
            });

            raeder_thread.join().unwrap();
            writer_thread.join().unwrap();
        });
    }
}
