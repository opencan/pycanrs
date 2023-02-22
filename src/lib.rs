use pyo3::{
    intern,
    types::{IntoPyDict, PyCFunction, PyDict, PyTuple},
    Py, PyAny, PyErr, Python, ToPyObject,
};
use thiserror::Error;

pub mod message;
pub use message::PyCanMessage;

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
    notifier: Py<PyAny>,
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
    #[error("Failed to add listener :: `{0}")]
    FailedToAddListener(String),
}

impl PyCanInterface {
    pub fn new(kind: PyCanBusType) -> Result<Self, PyCanError> {
        // Import python-can
        let pycan = Python::with_gil(|py| -> Result<Py<PyAny>, PyCanError> {
            Ok(py
                .import("can")
                .map_err(|e| PyCanError::PythonCanImportFailed(e.to_string()))?
                .to_object(py))
        })?;

        // Set up interface
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

        // Set up notifier thread
        let notifier = Python::with_gil(|py| -> Result<_, PyCanError> {
            let args = [
                py_dict_entry!(py, "bus", iface.clone()),
                py_dict_entry!(py, "listeners", PyTuple::empty(py)), // no listeners to start
            ]
            .into_py_dict(py);

            // Register the notifier
            pycan
                .call_method(py, "Notifier", (), Some(args))
                .map_err(|e| PyCanError::FailedToCreateNotifier(e.to_string()))
        })?;

        Ok(Self {
            bustype: kind,
            iface,
            notifier,
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

    /// Register the provided callback to be called on future recieved messages
    /// on this interface.
    pub fn register_rx_callback<R, E>(&self, on_rx: R, on_error: E) -> Result<(), PyCanError>
    where
        R: Fn(&PyCanMessage) + Send + 'static,
        E: Fn(&PyErr) + Send + 'static,
    {
        Python::with_gil(|py| -> Result<(), PyCanError> {
            // Make a shim to extract the PyCanMessage and call the actual callback
            let rx_shim = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &PyTuple, _kwargs: Option<&PyDict>| {
                    let (msg,) = args.extract::<(PyCanMessage,)>().expect(
                        "PyCanMessage should always be extractable from \
                          argument to python-can listener callback",
                    );

                    on_rx(&msg);
                },
            )
            .expect("creation of listener rx callback shim should always succeed");

            // And another shim for on_error
            let error_shim = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &PyTuple, _kwargs: Option<&PyDict>| {
                    let err = PyErr::from_value(
                        args.get_item(0)
                            .expect("python-can should have passed exception as arg to on_error"),
                    );

                    on_error(&err);
                },
            )
            .expect("creation of listener on_error callback shim should always succeed");

            // Use type() to make an instance of a class inheriting can.Listener
            // Equivalent Python is like:
            // ```
            //     listener = type("PyCanRsListener", (can.Listener,), {
            //         "on_message_received": rx_shim,
            //         "on_error": error_shim
            //     })
            //     listener = listener()
            // ```
            // So we're doing:
            // ```
            //     base = (can.Listener,)
            //     methods = {"on_message_received": rx_shim, "on_error": error_shim}
            //     listener = type("PyCanRsListener", base, methods)()
            // ```

            let type_builtin = py
                .import("builtins")
                .expect("should be able to import builtins")
                .getattr("type")
                .expect("builtins should have type()");

            let base = (self
                .pycan
                .getattr(py, "Listener")
                .expect("python-can should have Listener"),)
                .to_object(py);

            let methods = [
                py_dict_entry!(py, "on_message_received", rx_shim),
                py_dict_entry!(py, "on_error", error_shim),
            ]
            .into_py_dict(py);

            let type_args = (
                "PyCanRsListener".to_object(py),
                base.to_object(py),
                methods.to_object(py),
            );

            // call type() and then call the result of that
            let listener = type_builtin
                .call1(type_args)
                .expect("should be able to create class deriving Listener")
                .call0()
                .unwrap();

            // Register the listener
            self.notifier
                .call_method1(py, "add_listener", (listener,))
                .map_err(|e| PyCanError::FailedToAddListener(e.to_string()))?;

            Ok(())
        })
    }
}
