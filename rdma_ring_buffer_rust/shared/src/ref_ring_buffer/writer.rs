use std::{
    cell::Ref,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use crate::atomic_extension::AtomicExtension;

use super::RefRingBuffer;

pub struct RingBufferWriter<'a, T> {
    ring_buffer: *mut RefRingBuffer<'a, T>,
    offset: usize,
    limit: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> RingBufferWriter<'a, T> {
    pub(super) fn reserve(ring_buffer: &'a mut RefRingBuffer<'a, T>, limit: usize) -> Self {
        let head = ring_buffer.head.load_acquire();
        let tail = ring_buffer.tail.load_acquire();

        let buffer_size = ring_buffer.buffer.len();

        let mut avaliable = buffer_size - (tail - head);

        let to_end = buffer_size - (tail % buffer_size);

        avaliable = if avaliable > to_end {
            to_end
        } else {
            avaliable
        };

        Self {
            ring_buffer,
            offset: tail,
            limit: avaliable.min(limit),
            _marker: PhantomData,
        }
    }
}

impl<T> Deref for RingBufferWriter<'_, T> {
    type Target = [MaybeUninit<T>];

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ring_buffer).buffer[self.offset..self.offset + self.limit] }
    }
}

impl<T> DerefMut for RingBufferWriter<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.ring_buffer).buffer[self.offset..self.offset + self.limit] }
    }
}

impl<T> Drop for RingBufferWriter<'_, T> {
    fn drop(&mut self) {
        unsafe {
            (*self.ring_buffer).tail.store(
                self.offset + self.limit,
                std::sync::atomic::Ordering::Release,
            );
        }
    }
}