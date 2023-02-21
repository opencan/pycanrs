use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use pycanrs::*;

#[derive(Subcommand)]
enum Bus {
    Slcan {
        serial_port: String,
        bitrate: u32,
    },
    Socketcand {
        host: String,
        port: u16,
        channel: String,
    },
}

#[derive(Parser)]
struct Args {
    #[clap(subcommand)]
    bus: Bus,
}

pub fn main() -> Result<()> {
    let args = Args::parse();

    let can = PyCanInterface::new(match args.bus {
        Bus::Slcan {
            serial_port,
            bitrate,
        } => PyCanBusType::Slcan {
            bitrate,
            serial_port,
        },
        Bus::Socketcand {
            host,
            port,
            channel,
        } => PyCanBusType::Socketcand {
            channel,
            host,
            port,
        },
    })?;

    let cb = |msg: &_| println!("{msg}");
    let err_cb = |err: &_| {
        eprintln!("{err}");
        std::process::exit(-1);
    };
    can.register_rx_callback(cb, err_cb)?;

    // handle Ctrl-c
    ctrlc::set_handler(|| std::process::exit(0))?;

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
