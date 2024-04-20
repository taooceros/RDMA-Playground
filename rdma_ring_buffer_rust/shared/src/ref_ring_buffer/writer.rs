use std::{
    cell::Ref,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use crate::atomic_extension::AtomicExtension;

use super::RefRingBuffer;

pub struct RingBufferWriter<'a, T> {
    ring_buffer: &'a mut RefRingBuffer<T>,
    offset: usize,
    limit: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: Copy + Send> RingBufferWriter<'a, T> {
    pub(super) fn try_reserve(ring_buffer: &'a mut RefRingBuffer<T>, limit: usize) -> Option<Self> {
        unsafe {
            let head = ring_buffer.head.as_ref().unwrap().load_acquire();
            let tail = ring_buffer.tail.as_ref().unwrap().load_acquire();

            let buffer_size = ring_buffer.buffer_size();

            let mut avaliable = buffer_size - (tail - head);

            let to_end = buffer_size - (tail % buffer_size);

            avaliable = if avaliable > to_end {
                to_end
            } else {
                avaliable
            };

            if avaliable < limit {
                return None;
            }

            Some(Self {
                ring_buffer,
                offset: tail,
                limit: tail + limit,
                _marker: PhantomData,
            })
        }
    }
}

impl<T: Copy + Send> Deref for RingBufferWriter<'_, T> {
    type Target = [MaybeUninit<T>];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let start = self.offset % self.ring_buffer.buffer_size();
            let end = (self.offset + self.limit) % self.ring_buffer.buffer_size();
            if self.offset > 0 && end == 0 {
                &self.ring_buffer.buffer.as_ref().unwrap()[start..]
            } else {
                &self.ring_buffer.buffer.as_ref().unwrap()[start..end]
            }
        }
    }
}

impl<T: Copy + Send> DerefMut for RingBufferWriter<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            let start = self.offset % (*self.ring_buffer).buffer_size();
            let end = (self.offset + self.limit) % (*self.ring_buffer).buffer_size();
            if self.offset > 0 && end == 0 {
                &mut self.ring_buffer.buffer.as_mut().unwrap()[start..]
            } else {
                &mut self.ring_buffer.buffer.as_mut().unwrap()[start..end]
            }
        }
    }
}

impl<T> Drop for RingBufferWriter<'_, T> {
    fn drop(&mut self) {
        unsafe {
            (*self.ring_buffer).tail.as_ref().unwrap().store(
                self.offset + self.limit,
                std::sync::atomic::Ordering::Release,
            );
        }
    }
}
