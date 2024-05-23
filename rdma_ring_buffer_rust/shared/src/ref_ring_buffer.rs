use core::panic;
use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr,
    sync::{atomic::AtomicUsize, Mutex},
};

use uninit::{extension_traits::MaybeUninitExt, AsMaybeUninit};

use crate::atomic_extension::AtomicExtension;

use self::{receiver::Receiver, sender::Sender};

pub mod reader_chunk;
pub mod receiver;
pub mod sender;
pub mod writer_chunk;

// Safety: The Ref must not outlive the underlying RingBuffer
#[derive(Debug, Clone)]
pub struct RefRingBuffer<T> {
    head: *const AtomicUsize,
    tail: *const AtomicUsize,
    buffer: *mut [MaybeUninit<T>],
}

unsafe impl<T: Send> Send for RefRingBuffer<T> {}
unsafe impl<T: Send> Sync for RefRingBuffer<T> {}

impl<T> RefRingBuffer<T> {
    #[inline(always)]
    pub(super) fn buffer_size(&self) -> usize {
        unsafe { self.buffer.as_ref().unwrap_unchecked().len() }
    }

    #[inline(always)]
    pub(super) fn head_ref(&self) -> &AtomicUsize {
        unsafe { self.head.as_ref().unwrap_unchecked() }
    }

    #[inline(always)]
    pub(super) fn tail_ref(&self) -> &AtomicUsize {
        unsafe { self.tail.as_ref().unwrap_unchecked() }
    }
}

impl<T: Send + Copy> RefRingBuffer<T> {
    pub fn from_raw_parts(
        head: &AtomicUsize,
        tail: &AtomicUsize,
        buffer: *mut [MaybeUninit<T>],
    ) -> Self {
        Self { head, tail, buffer }
    }

    pub fn buffer_slice(&self) -> &mut [MaybeUninit<T>] {
        unsafe { self.buffer.as_mut().unwrap_unchecked() }
    }

    pub fn split(&mut self) -> (Sender<T>, Receiver<T>) {
        let sender = Sender::new(self);
        let receiver = Receiver::new(self);

        (sender, receiver)
    }

    // This writer will only return continuous memory slice regardless of the buffer is wrapped around
    pub fn reserve_write(&self, len: usize) -> Option<writer_chunk::WriteChunk<T>> {
        writer_chunk::WriteChunk::try_reserve(self, len)
    }
}
