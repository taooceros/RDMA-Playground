use std::num::NonZeroI32;

use clap::{arg, command, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct GlobalArgs {
    #[command(subcommand)]
    pub command: ConnectionType,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConnectionType {
    Client,
    Server,
}
