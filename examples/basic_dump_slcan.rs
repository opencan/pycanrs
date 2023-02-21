use std::time::Duration;

use anyhow::Result;
use clap::{Parser};
use pycanrs::*;

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    serial_port: String,
    #[clap(short, long)]
    bitrate: u32
}

pub fn main() -> Result<()> {
    let args = Args::parse();

    let can = PyCanInterface::new(PyCanBusType::Slcan { bitrate: args.bitrate, serial_port: args.serial_port })?;

    let cb = |msg: &_| println!("{msg}");
    can.recv_spawn(cb)?;

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
