use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about, long_about = None, version)]
pub struct Args {
    /// Address to listen on.
    #[arg(short, long, default_value_t = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))]
    pub listen: IpAddr,
    /// (TCP) port to listen on.
    #[arg(short, long, default_value_t = 11300)]
    pub port: u16,
    /// Enables write-ahead logging and set the directory to store WAL files in.
    #[arg(short = 'b', long)]
    pub wal_dir: Option<PathBuf>,
    /// Sets the maximum allowed job size.
    #[arg(short = 'z', long, default_value_t = 65535)]
    pub max_job_size: u32,
    /// Enables human-friendly logging.
    #[arg(short, long, default_value_t)]
    pub debug: bool,
}
