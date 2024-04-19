use core::panic;
use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::atomic_extension::AtomicExtension;

use self::writer::RingBufferWriter;

pub struct RefRingBuffer<'a, T> {
    head: &'a AtomicUsize,
    tail: &'a AtomicUsize,
    buffer: &'a mut [MaybeUninit<T>],
}

impl<'a, T: Send + Copy> RefRingBuffer<'a, T> {
    pub fn from_raw_parts(
        head: &'a AtomicUsize,
        tail: &'a AtomicUsize,
        buffer: &'a mut [MaybeUninit<T>],
    ) -> Self {
        Self { head, tail, buffer }
    }

    // The reader will only return continuous memory slice regardless of the buffer is wrapped around
    // This ensure that RingBufferReader can be converted into slice
    pub fn read(&mut self) -> reader::RingBufferReader<T> {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();
        let buffer_size = self.buffer.len();

        let mut avaliable = tail - head;

        avaliable = avaliable.min(buffer_size - (head % buffer_size));
        // SAFETY: acquire load for tail will ensure that the data is written before this line
        reader::RingBufferReader {
            ring_buffer: self,
            offset: head,
            limit: head + avaliable,
        }
    }

    // The writer doesn't ensure that the data written is continuous
    pub fn write(&mut self, data: &[T]) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let buffer_size = self.buffer.len();

        let mut avaliable = buffer_size - (tail - head);

        let to_end = buffer_size - (tail % buffer_size);

        avaliable = if avaliable > to_end {
            to_end
        } else {
            avaliable
        };

        let write_len = data.len().min(avaliable);

        let start = tail % buffer_size;

        unsafe {
            if start + write_len <= buffer_size {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    &mut self.buffer[start] as *mut MaybeUninit<T> as *mut T,
                    write_len,
                );
            } else {
                let end = buffer_size - start;
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    &mut self.buffer[start] as *mut MaybeUninit<T> as *mut T,
                    end,
                );
                ptr::copy_nonoverlapping(
                    data.as_ptr().add(end),
                    &mut self.buffer[0] as *mut MaybeUninit<T> as *mut T,
                    write_len - end,
                );
            }
        }

        self.tail.store_release(tail + write_len);

        write_len
    }

    // This writer will only return continuous memory slice regardless of the buffer is wrapped around
    pub fn alloc_write(&'a mut self, len: usize) -> RingBufferWriter<'a, T> {
        RingBufferWriter::reserve(self, len)
    }
}

mod reader;

mod writer;
