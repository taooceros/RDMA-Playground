use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub mod ring_buffer_metadata;

pub struct Ipc {
    pipe: File,
}

impl Ipc {
    pub fn create(path: impl AsRef<Path>) -> Ipc {
        let path = path.as_ref();
        if path.exists() {
            fs::remove_file(path).unwrap();
            println!("Removed existing fifo");
        }

        let c_name = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let c_name_ptr = c_name.as_ptr();
        let ret = unsafe { nix::libc::mkfifo(c_name_ptr, 0o666) };

        if ret < 0 {
            panic!("Failed to create fifo");
        }

        Ipc {
            pipe: OpenOptions::new().write(true).open(path).unwrap(),
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Ipc {
        let path = path.as_ref();

        while !path.exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ipc {
            pipe: OpenOptions::new().read(true).open(path).unwrap(),
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
