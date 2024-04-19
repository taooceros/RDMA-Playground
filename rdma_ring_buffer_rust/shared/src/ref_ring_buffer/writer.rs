use std::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use super::RefRingBuffer;

pub struct RingBufferWriter<'a, 'b, T>
where
    'a: 'b,
{
    pub(crate) ring_buffer: &'b mut RefRingBuffer<'a, T>,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
}

impl<T> Deref for RingBufferWriter<'_, '_, T> {
    type Target = [MaybeUninit<T>];

    fn deref(&self) -> &Self::Target {
        &self.ring_buffer.buffer[self.offset..self.offset + self.limit]
    }
}

impl<T> DerefMut for RingBufferWriter<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ring_buffer.buffer[self.offset..self.offset + self.limit]
    }
}

impl<T> Drop for RingBufferWriter<'_, '_, T> {
    fn drop(&mut self) {
        self.ring_buffer.tail.store(
            self.offset + self.limit,
            std::sync::atomic::Ordering::Release,
        );
    }
}
