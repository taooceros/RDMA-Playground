use std::num::NonZeroI32;

use clap::{arg, command, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct GlobalArgs {
    #[arg(short, long, default_value = "mlx5_0")]
    pub dev: String,
    #[arg(short, long, default_value = "1")]
    pub gid_index: Option<NonZeroI32>,
    #[arg(short, long)]
    pub server_addr: Option<String>,
    #[arg(short, long)]
    pub port: Option<u16>,
    #[arg(short, long, default_value_t = 64usize)]
    pub message_size: usize,
}
