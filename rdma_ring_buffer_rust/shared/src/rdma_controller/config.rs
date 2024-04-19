use std::{net::IpAddr, num::NonZeroI32};

pub struct Config {
    pub dev_name: String,
    pub gid_index: Option<NonZeroI32>,
    pub connection_type: ConnectionType,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionType {
    Server {
        port: u16,
        message_size: usize,
    },
    Client {
        server_addr: IpAddr,
        port: u16,
        message_size: usize,
    },
}
