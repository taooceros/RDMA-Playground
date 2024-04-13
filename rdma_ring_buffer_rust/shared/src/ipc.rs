use std::{
    fs::File,
    io::{Read, Write},
};

pub mod ring_buffer_metadata;

pub struct Ipc {
    pipe: File,
}

impl Ipc {
    pub fn new(name: &str) -> Ipc {
        let c_name = std::ffi::CString::new(name).unwrap();
        let c_name_ptr = c_name.as_ptr();
        let ret = unsafe { nix::libc::mkfifo(c_name_ptr, 0o666) };

        if ret < 0 {
            panic!("Failed to create fifo");
        }

        Ipc {
            pipe: File::open(name).unwrap(),
        }
    }
}

impl Read for Ipc {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.pipe.read(buf)
    }
}

impl Write for Ipc {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pipe.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.pipe.flush()
    }
}