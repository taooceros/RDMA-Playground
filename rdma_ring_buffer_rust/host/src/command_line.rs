use std::num::NonZeroI32;

use clap::{arg, command, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct GlobalArgs {
}

pub enum ConnectionType {
    Client,
    Server,
}
