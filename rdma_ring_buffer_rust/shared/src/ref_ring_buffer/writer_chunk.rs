use std::{
    cell::Ref,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use crate::atomic_extension::AtomicExtension;

use super::RefRingBuffer;

pub struct WriteChunk<'a, T> {
    ring_buffer: &'a RefRingBuffer<T>,
    start: usize,
    end: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: Copy + Send> WriteChunk<'a, T> {
    pub(super) fn try_reserve(ring_buffer: &'a RefRingBuffer<T>, size: usize) -> Option<Self> {
        unsafe {
            let head = ring_buffer.head_ref().load_acquire();
            let tail = ring_buffer.tail_ref().load_acquire();

            let buffer_size = ring_buffer.buffer_size();

            let mut avaliable = buffer_size - (tail - head);

            let to_end = buffer_size - (tail % buffer_size);

            avaliable = avaliable.min(to_end);

            if avaliable < size {
                return None;
            }

            Some(Self {
                ring_buffer,
                start: tail,
                end: tail + size,
                _marker: PhantomData,
            })
        }
    }
}

impl<T: Copy + Send> Deref for WriteChunk<'_, T> {
    type Target = [MaybeUninit<T>];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let start = self.start % self.ring_buffer.buffer_size();
            let length = self.end - self.start;
            &self.ring_buffer.buffer.as_mut().unwrap()[start..start + length]
        }
    }
}

impl<T: Copy + Send> DerefMut for WriteChunk<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let start = self.start % self.ring_buffer.buffer_size();
            let length = self.end - self.start;
            &mut self.ring_buffer.buffer.as_mut().unwrap()[start..start + length]
        }
    }
}

impl<T> WriteChunk<'_, T> {
    pub fn commit(&mut self) {
        unsafe {
            (*self.ring_buffer)
                .tail
                .as_ref()
                .unwrap()
                .store(self.end, std::sync::atomic::Ordering::Release);
        }
    }
}
