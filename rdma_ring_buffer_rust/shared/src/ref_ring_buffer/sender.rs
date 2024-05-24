use std::{mem::MaybeUninit, ptr};

use crate::atomic_extension::AtomicExtension;

use super::{writer_chunk::WriteChunk, RefRingBuffer};

pub struct Sender<'a, T> {
    ring_buffer: &'a RefRingBuffer<T>,
}

impl<'a, T> Sender<'a, T> {
    pub(super) fn new(ring_buffer: &'a RefRingBuffer<T>) -> Self {
        Self { ring_buffer }
    }

    pub fn ring_buffer(&self) -> &RefRingBuffer<T> {
        self.ring_buffer
    }
}

impl<'a, T: Copy + Send> Sender<'a, T> {
    pub fn try_reserve(&self, size: usize) -> Option<WriteChunk<'a, T>> {
        WriteChunk::try_reserve(self.ring_buffer, size)
    }

    // The writer doesn't ensure that the data written is continuous
    pub fn write(&self, data: &[T]) -> usize {
        unsafe {
            let head = self.ring_buffer.head_ref().load_acquire();
            let tail = self.ring_buffer.tail_ref().load_acquire();

            let buffer_size = self.ring_buffer.buffer_size();

            let avaliable = buffer_size - (tail - head);

            let write_len = data.len().min(avaliable);

            if write_len == 0 {
                return 0;
            }

            let start = tail % buffer_size;

            unsafe {
                if start + write_len <= buffer_size {
                    ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        &mut self.ring_buffer.buffer.as_mut().unwrap()[start] as *mut MaybeUninit<T> as *mut T,
                        write_len,
                    );
                } else {
                    let end = buffer_size - start;
                    ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        self.ring_buffer.buffer.as_mut().unwrap().as_mut_ptr().add(start) as *mut T,
                        end,
                    );
                    ptr::copy_nonoverlapping(
                        data.as_ptr().add(end),
                        self.ring_buffer.buffer.as_mut().unwrap().as_mut_ptr() as *mut T,
                        write_len - end,
                    );
                }
            }

            self.ring_buffer.tail_ref().store_release(tail + write_len);

            write_len
        }
    }
}