use std::{mem::MaybeUninit, sync::atomic::AtomicUsize};

use crate::atomic_extension::AtomicExtension;

pub struct RingBuffer<'a, T: Send, const Capacity: usize> {
    head: AtomicUsize,
    tail: AtomicUsize,
    buffer: &'a [MaybeUninit<T>; Capacity],
}

impl<T: Send, const Capacity: usize> RingBuffer<'_, T, Capacity> {
    pub fn new_alloc() -> RingBuffer<'static, T, Capacity> {
        unsafe {
            RingBuffer {
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                buffer: Box::leak(Box::new(MaybeUninit::uninit().assume_init())),
            }
        }
    }

    fn readpos_from_head(head: usize) {
        return head % Capacity;
    }

    pub fn read(&self, buffer: &mut [T]) -> u32 {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let avaliable = tail - head;

        

        avaliable.into()
    }
}
