use std::{mem::MaybeUninit, ptr, sync::atomic::AtomicUsize};

use crate::atomic_extension::AtomicExtension;

pub struct RingBuffer<'a, T, const N: usize> {
    head: AtomicUsize,
    tail: AtomicUsize,
    buffer: &'a mut [MaybeUninit<T>; N],
}

impl<'a, T: Send + Copy + Sized, const N: usize> RingBuffer<'a, T, N> {
    pub fn new_alloc() -> RingBuffer<'static, T, N> {
        unsafe {
            RingBuffer {
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                buffer: Box::leak(Box::new(MaybeUninit::uninit().assume_init())),
            }
        }
    }

    pub fn new(&mut self, buffer: &'a mut [MaybeUninit<T>; N]) -> RingBuffer<T, N> {
        RingBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer,
        }
    }

    pub fn read(&mut self, buffer: &mut [MaybeUninit<T>], count: usize) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let mut avaliable = tail - head;

        if count > 0 && avaliable > count {
            avaliable = count;
        }

        if avaliable == 0 {
            return 0;
        }

        let readpos = head % N;

        let _to_end = N - readpos;

        for i in 0..avaliable {
            buffer[i] = self.buffer[(readpos + i) % N];
        }

        self.head.store_release(head + avaliable);

        avaliable
    }

    pub fn write(&mut self, buffer: &mut [T], mut write_size: usize) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let write_pos = tail % N;

        let avaliable_space = N - (tail - head);

        if write_size > avaliable_space {
            write_size = avaliable_space;
        }

        let to_end = N - write_pos;

        for i in 0..write_size {
            self.buffer[(write_pos + i) % N] = MaybeUninit::new(buffer[i]);
        }

        self.tail.store_release(tail + write_size);

        write_size
    }

    pub fn read_avaliable(&self) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        tail - head
    }

    pub fn write_avaliable(&self) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        N - (tail - head)
    }
}
