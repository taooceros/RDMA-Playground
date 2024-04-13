
use std::{
    mem::{MaybeUninit},
    sync::atomic::AtomicUsize,
};

use crate::ref_ring_buffer::RefRingBuffer;

pub struct RingBuffer<T, const N: usize> {
    head: AtomicUsize,
    tail: AtomicUsize,
    buffer: [MaybeUninit<T>; N],
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
        RefRingBuffer::from_raw_parts(&self.head, &self.tail, &mut self.buffer)
    }
}
