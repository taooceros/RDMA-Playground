use core::panic;
use std::{mem::MaybeUninit, ptr, sync::atomic::AtomicUsize};

use crate::{atomic_extension::AtomicExtension, communication_manager::CommunicationManager};

pub struct RingBuffer<'a, T, const N: usize, CM: CommunicationManager> {
    head: AtomicUsize,
    tail: AtomicUsize,
    buffer: &'a mut [MaybeUninit<T>; N],
    communication_manager: &'a mut CM,
}

impl<'a, T: Send + Copy + Sized, const N: usize, CM: CommunicationManager>
    RingBuffer<'a, T, N, CM>
{
    pub fn new_alloc(communication_manager: &'a mut CM) -> RingBuffer<'a, T, N, CM> {
        unsafe {
            RingBuffer {
                head: AtomicUsize::new(0),
                tail: AtomicUsize::new(0),
                buffer: Box::leak(Box::new(MaybeUninit::uninit().assume_init())),
                communication_manager: communication_manager,
            }
        }
    }

    pub fn new(
        &mut self,
        buffer: &'a mut [MaybeUninit<T>; N],
        communication_manager: &'a mut CM,
    ) -> RingBuffer<T, N, CM> {
        RingBuffer {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            buffer,
            communication_manager,
        }
    }

    pub fn read(&mut self, buffer: &mut [MaybeUninit<T>], count: usize) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let messages = self
            .communication_manager
            .recv_message::<Message<T>>()
            .unwrap();

        for message in messages {
            if let Message::Write { data } = message {
                let write_pos = tail % N;
                self.buffer[write_pos] = MaybeUninit::new(*data);
                self.tail.store_release(tail + 1);
            } else {
                panic!("Only One Reader should exists");
            }
        }

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

        self.communication_manager
            .send_message(&[Message::Read::<T>(avaliable)])
            .unwrap();

        avaliable
    }

    pub fn write(&mut self, buffer: &[T], mut write_size: usize) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        let messages = self
            .communication_manager
            .recv_message::<Message<T>>()
            .unwrap();

        for message in messages {
            if let Message::Read(size) = message {
                self.head.store_release(head + size);
            } else {
                panic!("Only One Writer should exists");
            }
        }

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

        let mut send_buffer = Vec::with_capacity(write_size);

        for i in 0..write_size {
            send_buffer.push(Message::Write {
                data: buffer[i],
            });
        }

        self.communication_manager
            .send_message(&send_buffer)
            .unwrap();

        write_size
    }

    pub fn avaliable_read(&self) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        tail - head
    }

    pub fn avaliable_write(&self) -> usize {
        let head = self.head.load_acquire();
        let tail = self.tail.load_acquire();

        N - (tail - head)
    }
}

pub enum Message<T> {
    Read(usize),
    Write { data: T },
}
