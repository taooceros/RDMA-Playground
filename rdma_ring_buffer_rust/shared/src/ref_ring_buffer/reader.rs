use std::{mem::transmute, ops::Deref};

use super::RefRingBuffer;

pub struct RingBufferReader<'a, T> {
    pub(crate) ring_buffer: &'a RefRingBuffer<T>,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
}

impl<'a, T: Send + Copy> Iterator for RingBufferReader<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.limit {
            let item = unsafe {
                self.ring_buffer.buffer.as_mut().unwrap()
                    [self.offset % self.ring_buffer.buffer_size()]
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
        unsafe {
            self.ring_buffer
                .head
                .as_ref()
                .unwrap()
                .store(self.limit, std::sync::atomic::Ordering::Release);
        }
    }
}

impl<T: Send + Copy> Deref for RingBufferReader<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        let start = self.offset % self.ring_buffer.buffer_size();
        let end = self.limit % self.ring_buffer.buffer_size();

        if self.limit > 0 && end == 0 {
            unsafe { transmute(&(self.ring_buffer.buffer.as_ref().unwrap()[start..])) }
        } else {
            unsafe { transmute(&(self.ring_buffer.buffer.as_ref().unwrap()[start..end])) }
        }
    }
}
