use std::io::Write;

#[derive(Debug)]
pub struct RingBufferMetaData {
    pub ring_buffer_len: usize,
    pub shared_memory_name: Vec<u8>,
}

impl RingBufferMetaData {
    pub fn write_to_ipc(&self, mut writer: impl Write) {
        writer
            .write(self.ring_buffer_len.to_le_bytes().as_ref())
            .unwrap();

        writer.write(&self.shared_memory_name).unwrap();

        writer.flush().unwrap();
    }

    pub fn read_from_ipc(mut reader: impl std::io::Read) -> Box<Self> {
        let mut ring_buffer_len_bytes = [0u8; 8];
        reader.read_exact(&mut ring_buffer_len_bytes).unwrap();
        let ring_buffer_len = usize::from_le_bytes(ring_buffer_len_bytes);

        let mut shared_memory_name = [0u8; 256];
        let len = reader.read(&mut shared_memory_name).unwrap();

        Box::new(RingBufferMetaData {
            ring_buffer_len,
            shared_memory_name: shared_memory_name[..len].to_owned(),
        })
    }
}
