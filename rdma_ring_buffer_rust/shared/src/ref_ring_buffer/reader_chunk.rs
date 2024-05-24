use std::fmt::Debug;
use std::{fmt::Formatter, mem::transmute, ops::Deref};

use super::RefRingBuffer;

pub struct ReadChunk<'a, T: Copy + Send> {
    pub ring_buffer: &'a RefRingBuffer<T>,
    pub start: usize,
    pub end: usize,
}

impl<T: Copy + Send + Debug> Debug for ReadChunk<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingBufferReader")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("buffer", &self.deref())
            .finish()
    }
}

impl<T: Send + Copy> ReadChunk<'_, T> {
    pub fn commit(&mut self) {
        self.ring_buffer
            .head_ref()
            .store(self.end, std::sync::atomic::Ordering::Release);
    }
}

impl<T: Send + Copy> Deref for ReadChunk<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        let start = self.start % self.ring_buffer.buffer_size();
        let length = self.end - self.start;

        unsafe {
            transmute::<&[std::mem::MaybeUninit<T>], &[T]>(
                &self.ring_buffer.buffer.as_mut().unwrap()[start..start + length],
            )
        }
    }
}
