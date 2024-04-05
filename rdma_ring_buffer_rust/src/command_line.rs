use clap::{arg, command, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct GlobalArgs {
    #[arg(short, long, default_value = "rocep152s0f0")]
    pub dev: String,
    #[arg(short, long)]
    pub server_addr: Option<String>,
    #[arg(short, long)]
    pub port: Option<u16>,
}
