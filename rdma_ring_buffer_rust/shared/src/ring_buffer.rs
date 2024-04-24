use std::{
    cell::UnsafeCell, mem::MaybeUninit, ops::{Deref, DerefMut}, sync::atomic::AtomicUsize
};

use crate::{atomic_extension::AtomicExtension, ref_ring_buffer::RefRingBuffer};

#[repr(C)]
pub struct RingBuffer<T, const N: usize> {
    pub head: AtomicUsize,
    pub tail: AtomicUsize,
    pub buffer: UnsafeCell<[MaybeUninit<T>; N]>,
}

impl<T: Send + Copy, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    pub fn to_ref(&mut self) -> RefRingBuffer<T> {
        RefRingBuffer::from_raw_parts(&self.head, &self.tail, self.buffer.get())
    }
}
