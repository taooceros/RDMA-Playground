use std::{mem::transmute, ops::Deref};

use super::RefRingBuffer;

pub struct RingBufferReader<'a, T> {
    pub(crate) ring_buffer: &'a RefRingBuffer<'a, T>,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
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

impl<T> Drop for RingBufferReader<'_, T> {
    fn drop(&mut self) {
        self.ring_buffer
            .head
            .store(self.offset, std::sync::atomic::Ordering::Release);
    }
}

impl<T> Deref for RingBufferReader<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { transmute(&(self.ring_buffer.buffer[self.offset..self.limit])) }
    }
}
