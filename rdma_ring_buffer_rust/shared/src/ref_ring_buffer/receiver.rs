use crate::atomic_extension::AtomicExtension;

use super::{reader_chunk::ReadChunk, RefRingBuffer};

pub struct Receiver<'a, T> {
    ring_buffer: &'a RefRingBuffer<T>
}

impl<'a, T> Receiver<'a, T> {
    pub(super) fn new(ring_buffer: &'a RefRingBuffer<T>) -> Self {
        Self { ring_buffer }
    }

    pub fn ring_buffer(&self) -> &RefRingBuffer<T> {
        self.ring_buffer
    }
}

impl<'a, T: Copy + Send> Receiver<'a, T> {
    pub fn read_exact(&self, len: usize) -> Option<ReadChunk<T>> {
        unsafe {
            let head = self.ring_buffer.head_ref().load_acquire();
            let tail = self.ring_buffer.tail_ref().load_acquire();
            let buffer_size = self.ring_buffer.buffer_size();

            let mut avaliable = tail - head;

            let to_end = buffer_size - (head % buffer_size);

            avaliable = avaliable.min(to_end);

            if avaliable < len {
                return None;
            }

            Some(ReadChunk {
                ring_buffer: self.ring_buffer,
                start: head,
                end: head + len,
            })
        }
    }

    // The reader will only return continuous memory slice regardless of the buffer is wrapped around
    // This ensure that RingBufferReader can be converted into slice
    pub fn read(&self) -> ReadChunk<T> {
        unsafe {
            let head = self.ring_buffer.head_ref().load_acquire();
            let tail = self.ring_buffer.tail_ref().load_acquire();
            let buffer_size = self.ring_buffer.buffer_size();

            let mut avaliable = tail - head;

            avaliable = avaliable.min(buffer_size - (head % buffer_size));
            // SAFETY: acquire load for tail will ensure that the data is written before this line
            ReadChunk {
                ring_buffer: self.ring_buffer,
                start: head,
                end: head + avaliable,
            }
        }
    }
}