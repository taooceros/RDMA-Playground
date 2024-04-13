use core::panic;
use std::{
    mem::{transmute, MaybeUninit},
    ptr,
    sync::{atomic::AtomicUsize, Mutex},
};

use crate::atomic_extension::AtomicExtension;

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
    pub fn read(&mut self) -> RingBufferReader<T> {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();
        let buffer_size = self.buffer.len();

        let mut avaliable = tail - head;

        avaliable = avaliable.min(buffer_size - (head % buffer_size));
        // SAFETY: acquire load for tail will ensure that the data is written before this line
        RingBufferReader {
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
}

pub struct RingBufferReader<'a, T> {
    ring_buffer: &'a RefRingBuffer<'a, T>,
    offset: usize,
    limit: usize,
}

impl<'a, T> Iterator for RingBufferReader<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.limit {
            let item = unsafe {
                self.ring_buffer.buffer[self.offset % self.ring_buffer.buffer.len()]
                    .assume_init_ref()
            };
            self.offset += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.limit - self.offset, Some(self.limit - self.offset))
    }
}

impl<'a, T> RingBufferReader<'a, T> {
    pub fn as_slice(&self) -> &'a [T] {
        let buffer_size = self.ring_buffer.buffer.len();
        let start = self.offset % buffer_size;
        let end = self.limit % buffer_size;
        if start < end {
            unsafe { transmute(&self.ring_buffer.buffer[start..end]) }
        } else {
            unsafe { transmute(&self.ring_buffer.buffer[start..buffer_size]) }
        }
    }

    pub fn len(&self) -> usize {
        self.limit - self.offset
    }
}

impl<T> Drop for RingBufferReader<'_, T> {
    fn drop(&mut self) {
        self.ring_buffer
            .head
            .store(self.limit, std::sync::atomic::Ordering::Release);
    }
}
