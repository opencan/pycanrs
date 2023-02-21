use pyo3::{
    intern,
    types::{IntoPyDict, PyCFunction, PyDict, PyTuple},
    Py, PyAny, Python, ToPyObject,
};
use thiserror::Error;

pub mod message;
use message::PyCanMessage;

pub enum PyCanBusType {
    Gsusb {
        bitrate: u32,
        usb_channel: String,
        usb_bus: u32,
        usb_address: u32,
    },
    Slcan {
        bitrate: u32,
        serial_port: String,
    },
    Socketcan {
        channel: String,
    },
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

#[derive(Debug, Error)]
pub enum PyCanError {
    #[error("Failed to import python-can - is it installed? :: `{0}`")]
    PythonCanImportFailed(String),
    #[error("Failed to create python-can interface :: `{0}`")]
    FailedToCreateInterface(String),
    #[error("Failed to create notifier :: `{0}`")]
    FailedToCreateNotifier(String),
}

impl PyCanInterface {
    pub fn new(kind: PyCanBusType) -> Result<Self, PyCanError> {
        let pycan = Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
            Ok(py
                .import("can")
                .map_err(|e| PyCanError::PythonCanImportFailed(e.to_string()))?
                .to_object(py))
        })?;

        let iface = match &kind {
            PyCanBusType::Gsusb {
                bitrate,
                usb_channel,
                usb_bus,
                usb_address,
            } => Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
                // Note: issues finding libusb on Mac - see:
                // https://github.com/pyusb/pyusb/issues/355#issuecomment-1214444040
                // We might have to manually look up libusb to help

                let args = [
                    py_dict_entry!(py, "bustype", "gs_usb"),
                    py_dict_entry!(py, "bitrate", bitrate),
                    py_dict_entry!(py, "channel", usb_channel),
                    py_dict_entry!(py, "bus", usb_bus),
                    py_dict_entry!(py, "address", usb_address),
                ]
                .into_py_dict(py);

                let iface = pycan
                    .call_method(py, "Bus", (), Some(args))
                    .map_err(|e| PyCanError::FailedToCreateInterface(e.to_string()))?;

                Ok(iface)
            }),
            PyCanBusType::Slcan {
                bitrate,
                serial_port,
            } => Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
                let args = [
                    py_dict_entry!(py, "bustype", "slcan"),
                    py_dict_entry!(py, "channel", serial_port),
                    py_dict_entry!(py, "bitrate", bitrate),
                ]
                .into_py_dict(py);

                let iface = pycan
                    .call_method(py, "Bus", (), Some(args))
                    .map_err(|e| PyCanError::FailedToCreateInterface(e.to_string()))?;

                Ok(iface)
            }),
            PyCanBusType::Socketcan { channel } => {
                Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
                    let args = [
                        py_dict_entry!(py, "bustype", "socketcan"),
                        py_dict_entry!(py, "channel", channel),
                    ]
                    .into_py_dict(py);

                    let iface = pycan
                        .call_method(py, "Bus", (), Some(args))
                        .map_err(|e| PyCanError::FailedToCreateInterface(e.to_string()))?;

                    Ok(iface)
                })
            }
            PyCanBusType::Socketcand {
                host,
                channel,
                port,
            } => Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
                let args = [
                    py_dict_entry!(py, "bustype", "socketcand"),
                    py_dict_entry!(py, "host", host),
                    py_dict_entry!(py, "channel", channel),
                    py_dict_entry!(py, "port", port),
                ]
                .into_py_dict(py);

                let iface = pycan
                    .call_method(py, "Bus", (), Some(args))
                    .map_err(|e| PyCanError::FailedToCreateInterface(e.to_string()))?;

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

    /// Spawn a python-can Notifier to call the provided callback on future
    /// recieved messages on this interface.
    pub fn recv_spawn<F>(&self, callback: F) -> Result<(), PyCanError>
    where
        F: Fn(&PyCanMessage) + Send + 'static,
    {
        Python::with_gil(|py| -> Result<(), PyCanError> {
            // Make a shim to extract the PyCanMessage and call the actual callback
            let callback_shim = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &PyTuple, _kwargs: Option<&PyDict>| {
                    let (msg,) = args.extract::<(PyCanMessage,)>().expect(
                        "PyCanMessage should always be extractable from argument to python-can Notifier callback",
                    );

                    callback(&msg);
                },
            )
            .expect("creation of Notifier callback shim function should succeed");

            let args = [
                py_dict_entry!(py, "bus", self.iface.clone()),
                py_dict_entry!(py, "listeners", [callback_shim]),
            ]
            .into_py_dict(py);

            // Register the notifier
            self.pycan
                .call_method(py, "Notifier", (), Some(args))
                .map_err(|e| PyCanError::FailedToCreateNotifier(e.to_string()))?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        #[cfg(not(target_os = "linux"))]
        let can = PyCanInterface::new(PyCanBusType::Socketcand {
            host: "side".into(),
            channel: "vcan0".into(),
            port: 30000,
        })
        .unwrap();

        #[cfg(target_os = "linux")]
        let can = PyCanInterface::new(PyCanBusType::Socketcan {
            channel: "vcan0".into(),
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
        // #[cfg(not(target_os = "linux"))]
        // let can = PyCanInterface::new(PyCanBusType::Socketcand {
        //     host: "side".into(),
        //     channel: "vcan0".into(),
        //     port: 30000,
        // })
        // .unwrap();

        // #[cfg(target_os = "linux")]
        // let can = PyCanInterface::new(PyCanBusType::Socketcan {
        //     channel: "vcan0".into(),
        // })
        // .unwrap();

        // Python::with_gil(|py| {
        //     println!("{}", py.version());
        // });

        // let can = PyCanInterface::new(PyCanBusType::Gsusb {
        //     bitrate: 500000,
        //     usb_channel: "canable gs_usb".into(),
        //     usb_bus: 0,
        //     usb_address: 4
        // }).unwrap();

        let can = PyCanInterface::new(PyCanBusType::Slcan {
            bitrate: 500000,
            serial_port: "/dev/tty.usbmodem1201".into(),
        })
        .unwrap();

        let cb_print = |msg: &PyCanMessage| println!("recv by callback!: {msg}");

        can.recv_spawn(cb_print).unwrap();
        loop {}
    }
}
