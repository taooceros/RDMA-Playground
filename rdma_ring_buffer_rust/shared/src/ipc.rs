use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::net::{SocketAddr, UnixListener, UnixStream},
    path::Path,
};

pub mod ring_buffer_metadata;

pub struct Ipc {
    pipe: UnixStream,
}

impl Ipc {
    pub fn create(path: impl AsRef<Path>) -> Ipc {
        let listener = UnixListener::bind(path).unwrap();

        Ipc {
            pipe: listener.accept().unwrap().0,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Ipc {
        while !path.as_ref().exists() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let stream = UnixStream::connect(path).unwrap();

        Ipc { pipe: stream }
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
