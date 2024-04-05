use std::net::IpAddr;

pub struct Config {
    pub dev_name: String,
    pub connection_type: ConnectionType,
}

pub enum ConnectionType {
    Server {
        port: u16
    },
    Client {
        server_addr: IpAddr,
        port: u16
    },
}
