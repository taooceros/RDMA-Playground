use std::{
    mem::{transmute, MaybeUninit},
    ptr,
    sync::atomic::AtomicUsize,
};

use crate::atomic_extension::AtomicExtension;

pub struct RefRingBuffer<'a, T> {
    head: &'a AtomicUsize,
    tail: &'a AtomicUsize,
    buffer: &'a mut [MaybeUninit<T>],
}

impl<'a, T: Send + Copy> RefRingBuffer<'a, T> {
    pub fn from_raw_parts(
        head: &'a AtomicUsize,
        tail: &'a AtomicUsize,
        buffer: &'a mut [MaybeUninit<T>],
    ) -> Self {
        Self { head, tail, buffer }
    }

    pub fn read(&self) -> &[T] {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();
        let buffer_size = self.buffer.len();

        if head == tail {
            panic!("RingBuffer is empty");
        }

        let mut avaliable = tail - head;
        let to_end = buffer_size - head;

        avaliable = if avaliable > to_end {
            to_end
        } else {
            avaliable
        };

        // SAFETY: acquire load for tail will ensure that the data is written before this line
        unsafe { transmute(&self.buffer[head..head + avaliable]) }
    }

    pub fn write(&mut self, data: &[T]) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let buffer_size = self.buffer.len();

        let mut avaliable = buffer_size - (tail - head);
        let to_end = buffer_size - tail;

        avaliable = if avaliable > to_end {
            to_end
        } else {
            avaliable
        };

        if avaliable < data.len() {
            panic!("RingBuffer is full");
        }

        for (i, item) in data.iter().enumerate() {
            self.buffer[(tail + i) % buffer_size].write(*item);
        }

        self.tail.store_release(tail + data.len());

        avaliable
    }
}
