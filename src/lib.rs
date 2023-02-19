use anyhow::Result;
use message::PyCanMessage;
use pyo3::{
    intern,
    types::{IntoPyDict, PyCFunction, PyDict, PyTuple},
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
    pycan: Py<PyAny>,
}

/// pyo3 dict entry.
/// Interns the key, converts the value to a PyObject.
macro_rules! py_dict_entry {
    ($py:expr, $x:expr, $y:expr) => {
        (intern!($py, $x), $y.to_object($py))
    };
}

impl PyCanInterface {
    pub fn new(kind: PyCanBusType) -> Result<Self> {
        let pycan = Python::with_gil(|py| -> Result<_> { Ok(py.import("can")?.to_object(py)) })?;

        let iface = match &kind {
            PyCanBusType::Socketcand {
                host,
                channel,
                port,
            } => Python::with_gil(|py| -> Result<_> {
                let args = [
                    py_dict_entry!(py, "bustype", "socketcand"),
                    py_dict_entry!(py, "host", host),
                    py_dict_entry!(py, "channel", channel),
                    py_dict_entry!(py, "port", port),
                ]
                .into_py_dict(py);

                let iface =
                    pycan
                        .getattr(py, "interface")?
                        .call_method(py, "Bus", (), Some(args))?;

                Ok(iface)
            }),
        }?;

        Ok(Self {
            bustype: kind,
            iface,
            pycan,
        })
    }

    pub fn recv(&self) -> PyCanMessage {
        Python::with_gil(|py| -> _ {
            self.iface
                .call_method0(py, intern!(py, "recv"))
                .unwrap()
                .extract(py)
                .unwrap()
        })
    }

    pub fn send(&self, id: u32, data: &[u8]) {
        Python::with_gil(|py| {
            let kwargs = [
                py_dict_entry!(py, "arbitration_id", id),
                py_dict_entry!(py, "data", data),
                py_dict_entry!(py, "dlc", data.len()),
            ]
            .into_py_dict(py);

            let msg = self
                .pycan
                .call_method(py, "Message", (), Some(kwargs))
                .unwrap();

            self.iface
                .call_method1(py, "send", PyTuple::new(py, [msg]))
                .unwrap();
        })
    }

    pub fn recv_spawn(&self) {
        Python::with_gil(|py| -> Result<()> {
            let callback = PyCFunction::new_closure(
                py,
                None,
                None,
                |args: &PyTuple, _kwargs: Option<&PyDict>| {
                    let (msg,) = args.extract::<(PyCanMessage,)>().unwrap();

                    println!("recv by callback: {msg}");
                },
            )?;

            let args = [
                py_dict_entry!(py, "bus", self.iface.clone()),
                py_dict_entry!(py, "listeners", [callback]),
            ]
            .into_py_dict(py);

            self.pycan.call_method(py, "Notifier", (), Some(args))?;

            Ok(())
        })
        .unwrap();
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

    #[test]
    fn test_spawn() {
        let can = PyCanInterface::new(PyCanBusType::Socketcand {
            host: "side".into(),
            channel: "vcan0".into(),
            port: 30000,
        })
        .unwrap();

        can.recv_spawn();
        loop {}
    }
}
