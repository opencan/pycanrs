use anyhow::Result;
use message::PyCanMessage;
use pyo3::{
    types::{IntoPyDict, PyDict, PyTuple},
    Py, PyAny, Python, ToPyObject, IntoPy,
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
    pycan: Py<PyAny>,
}

impl PyCanInterface {
    pub fn new(kind: PyCanBusType) -> Result<Self> {
        let (iface, pycan) = match &kind {
            PyCanBusType::Socketcand {
                host,
                channel,
                port,
            } => Python::with_gil(|py| -> Result<_> {
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

                Ok((iface.to_object(py), can.to_object(py)))
            }),
        }?;

        Ok(Self {
            bustype: kind,
            iface,
            pycan
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

    pub fn send(&self, id: u32, data: &[u8]) {
        Python::with_gil(|py| {
            let kwargs = [
                ("arbitration_id", id.to_object(py)),
                ("data", data.to_object(py)),
                ("dlc", data.len().to_object(py))
            ].into_py_dict(py);

            let msg = self.pycan.call_method(py, "Message", (), Some(kwargs)).unwrap();

            self.iface.call_method1(py, "send", PyTuple::new(py, [msg])).unwrap();
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

            can.send(message.arbitration_id, &message.data.unwrap());
        }
    }
}
