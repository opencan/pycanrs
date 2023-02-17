use std::fmt::Debug;

use anyhow::Result;
use pyo3::{
    prelude,
    types::{IntoPyDict, PyDict, PyModule},
    IntoPy, Py, Python,
};

pub struct PyCanInterface;

impl PyCanInterface {
    pub fn new_socketcan(dev: &str) {
        Python::with_gil(|py| -> Result<()> {
            let can = py.import("can")?;

            let args = PyDict::new(py);
            args.update(
                [
                    ("bustype", "socketcand"),
                    ("host", "side"),
                    ("channel", "vcan0"),
                ]
                .into_py_dict(py)
                .as_mapping(),
            )?;

            args.update([("port", 30000)].into_py_dict(py).as_mapping())?;

            let iface = can
                .getattr("interface")?
                .call_method("Bus", (), Some(args))?;

            print!("{iface:?}");

            loop {
                let message = iface.call_method0("recv")?;

                let print = message.to_string();
                println!("{message:?}");
            }
        })
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        PyCanInterface::new_socketcan("blah");
    }
}
