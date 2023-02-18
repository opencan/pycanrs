use anyhow::Result;
use message::PyCanMessage;
use pyo3::{
    types::{IntoPyDict, PyDict},
    Py, PyAny, Python, ToPyObject,
};

pub mod message;

pub enum PyCanBusType {
    Socketcand {
        host: String,
        channel: String,
        port: u16,
    },
}

pub struct PyCanInterface {
    pub bustype: PyCanBusType,
    iface: Py<PyAny>,
}

impl PyCanInterface {
    pub fn new(kind: PyCanBusType) -> Result<Self> {
        let iface = match &kind {
            PyCanBusType::Socketcand {
                host,
                channel,
                port,
            } => Python::with_gil(|py| -> Result<Py<PyAny>> {
                let can = py.import("can")?;

                let args = PyDict::new(py);
                args.update(
                    [
                        ("bustype", "socketcand"),
                        ("host", host),
                        ("channel", channel),
                    ]
                    .into_py_dict(py)
                    .as_mapping(),
                )?;

                args.update([("port", port)].into_py_dict(py).as_mapping())?;

                let iface = can
                    .getattr("interface")?
                    .call_method("Bus", (), Some(args))?;

                Ok(iface.to_object(py))
            }),
        }?;

        Ok(Self {
            bustype: kind,
            iface,
        })
    }

    pub fn recv(&self) -> PyCanMessage {
        Python::with_gil(|py| -> _ {
            self.iface
                .call_method0(py, "recv")
                .unwrap()
                .extract(py)
                .unwrap()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let can = PyCanInterface::new(PyCanBusType::Socketcand {
            host: "side".into(),
            channel: "vcan0".into(),
            port: 30000,
        })
        .unwrap();

        loop {
            let message = can.recv();
            println!("recv {message}");
        }
    }
}
