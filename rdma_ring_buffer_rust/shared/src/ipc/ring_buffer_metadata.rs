use std::{io::Write, mem::MaybeUninit};

use zerocopy::{AsBytes, FromBytes, FromZeroes};

#[derive(Debug, FromZeroes, FromBytes, AsBytes, Clone)]
#[repr(C)]
pub struct RingBufferMetaData {
    pub head_offset: usize,
    pub tail_offset: usize,
    pub buffer_offset: usize,
    pub ring_buffer_len: usize,
    pub shared_memory_name: [u8; 32],
}

impl RingBufferMetaData {
    pub fn write_to(&self, mut writer: impl Write) {
        writer.write_all(zerocopy::AsBytes::as_bytes(self)).unwrap();
    }

    pub fn read_from(mut reader: impl std::io::Read) -> Self {
        let mut buffer = [0u8; std::mem::size_of::<Self>()];
        reader.read_exact(&mut buffer).unwrap();
        zerocopy::FromBytes::read_from(&buffer).unwrap()
    }
}
