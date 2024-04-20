use std::num::{NonZeroU64, NonZeroUsize};

use clap::{arg, command, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct GlobalArgs {
    #[command(subcommand)]
    pub command: ConnectionType,
    #[arg(short, long, default_value_t = NonZeroUsize::new(64).unwrap())]
    pub batch_size: NonZeroUsize,
    #[arg(short, long, default_value_t = NonZeroU64::new(5).unwrap())]
    pub duration: NonZeroU64,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConnectionType {
    Client,
    Server,
}
