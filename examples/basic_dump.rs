use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use pycanrs::{*, message::PyCanMessage};

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

    let can = PyCanInterface::new(match &args.bus {
        Bus::Slcan {
            serial_port,
            bitrate,
        } => PyCanBusType::Slcan {
            bitrate: *bitrate,
            serial_port: serial_port.clone(),
        },
        Bus::Socketcand {
            host,
            port,
            channel,
        } => PyCanBusType::Socketcand {
            channel: channel.clone(),
            host: host.clone(),
            port: *port,
        },
    })?;

    let iface_name = match args.bus {
        Bus::Slcan { serial_port, .. } => serial_port,
        Bus::Socketcand { host, port, channel } => format!("{channel}@{host}:{port}"),
    };

    let cb = move |msg: &PyCanMessage| {
        let mut data = String::new();
        for byte in msg.data.as_ref().unwrap() {
            data += &format!("{:X} ", byte);
        }
        let data = data.trim();

        println!("  {iface_name}  {:08X}   [{}]  {data}",
                msg.arbitration_id, msg.dlc.unwrap());
    };

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
