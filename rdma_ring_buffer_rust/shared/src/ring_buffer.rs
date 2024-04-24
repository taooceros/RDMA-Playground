use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicUsize,
};

use crossbeam::utils::CachePadded;

use crate::{atomic_extension::AtomicExtension, ref_ring_buffer::RefRingBuffer};

#[repr(C, align(4096))]
pub struct RingBufferConst<T, const N: usize> {
    pub head: CachePadded<AtomicUsize>,
    pub tail: CachePadded<AtomicUsize>,
    pub buffer: UnsafeCell<[MaybeUninit<T>; N]>,
}

impl<T: Send + Copy, const N: usize> RingBufferConst<T, N> {
    pub fn new() -> Self {
        Self {
            head: AtomicUsize::new(0).into(),
            tail: AtomicUsize::new(0).into(),
            buffer: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    pub fn to_ref(&mut self) -> RefRingBuffer<T> {
        RefRingBuffer::from_raw_parts(&self.head, &self.tail, self.buffer.get())
    }
}

#[repr(C)]
pub struct RingBufferAlloc<T> {
    pub head: CachePadded<AtomicUsize>,
    pub tail: CachePadded<AtomicUsize>,
    pub buffer: Vec<MaybeUninit<T>>,
}

impl<T: Send + Copy> RingBufferAlloc<T> {
    pub fn new(size: usize) -> Self {
        Self {
            head: AtomicUsize::new(0).into(),
            tail: AtomicUsize::new(0).into(),
            buffer: vec![MaybeUninit::uninit(); size],
        }
    }

    pub fn to_ref(&mut self) -> RefRingBuffer<T> {
        RefRingBuffer::from_raw_parts(&self.head, &self.tail, &mut *self.buffer)
    }
}
