use core::panic;
use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr,
    sync::{atomic::AtomicUsize, Mutex},
};

use uninit::AsMaybeUninit;

use crate::atomic_extension::AtomicExtension;

use self::writer::RingBufferWriter;

// Safety: The Ref must not outlive the underlying RingBuffer
pub struct RefRingBuffer<T> {
    head: *const AtomicUsize,
    tail: *const AtomicUsize,
    buffer: *mut [MaybeUninit<T>],
}

impl<T: Send + Copy> RefRingBuffer<T> {
    pub fn buffer_size(&self) -> usize {
        unsafe { self.buffer.as_ref().unwrap().len() }
    }

    fn head_ref(&self) -> &AtomicUsize {
        unsafe { self.head.as_ref().unwrap() }
    }

    fn tail_ref(&self) -> &AtomicUsize {
        unsafe { self.tail.as_ref().unwrap() }
    }

    pub fn from_raw_parts(
        head: &AtomicUsize,
        tail: &AtomicUsize,
        buffer: &mut [MaybeUninit<T>],
    ) -> Self {
        Self { head, tail, buffer }
    }

    pub fn read_exact(&mut self, len: usize) -> Option<reader::RingBufferReader<T>> {
        unsafe {
            let head = self.head_ref().load_acquire();
            let tail = self.tail_ref().load_acquire();
            let buffer_size = self.buffer_size();

            let mut avaliable = tail - head;

            avaliable = avaliable.min(buffer_size - (head % buffer_size));

            if avaliable < len {
                return None;
            }

            // SAFETY: acquire load for tail will ensure that the data is written before this line
            Some(reader::RingBufferReader {
                ring_buffer: self,
                start: head,
                end: head + len,
            })
        }
    }

    // The reader will only return continuous memory slice regardless of the buffer is wrapped around
    // This ensure that RingBufferReader can be converted into slice
    pub fn read(&mut self) -> reader::RingBufferReader<T> {
        unsafe {
            let head = self.head_ref().load_acquire();
            let tail = self.tail_ref().load_acquire();
            let buffer_size = self.buffer_size();

            let mut avaliable = tail - head;

            avaliable = avaliable.min(buffer_size - (head % buffer_size));
            // SAFETY: acquire load for tail will ensure that the data is written before this line
            reader::RingBufferReader {
                ring_buffer: self,
                offset: head,
                limit: head + avaliable,
            }
        }
    }

    // The writer doesn't ensure that the data written is continuous
    pub fn write(&mut self, data: &[T]) -> usize {
        unsafe {
            let head = self.head_ref().load_acquire();
            let tail = self.tail_ref().load_acquire();

            let buffer_size = self.buffer_size();

            let mut avaliable = buffer_size - (tail - head);

            let to_end = buffer_size - (tail % buffer_size);

            avaliable = if avaliable > to_end {
                to_end
            } else {
                avaliable
            };

            let write_len = data.len().min(avaliable);

            let start = tail % buffer_size;

            unsafe {
                if start + write_len <= buffer_size {
                    ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        &mut self.buffer.as_mut().unwrap()[start] as *mut MaybeUninit<T> as *mut T,
                        write_len,
                    );
                } else {
                    let end = buffer_size - start;
                    ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        &mut self.buffer.as_mut().unwrap()[start] as *mut MaybeUninit<T> as *mut T,
                        end,
                    );
                    ptr::copy_nonoverlapping(
                        data.as_ptr().add(end),
                        &mut self.buffer.as_mut().unwrap()[0] as *mut MaybeUninit<T> as *mut T,
                        write_len - end,
                    );
                }
            }

            self.tail_ref().store_release(tail + write_len);

            write_len
        }
    }

    // This writer will only return continuous memory slice regardless of the buffer is wrapped around
    pub fn reserve_write(&mut self, len: usize) -> Option<RingBufferWriter<T>> {
        RingBufferWriter::try_reserve(self, len)
    }
}

mod reader;

mod writer;
