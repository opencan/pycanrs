use pyo3::prelude::*;
use std::fmt::{Debug, Display};

#[derive(Debug, FromPyObject)]
pub struct PyCanMessage {
    pub arbitration_id: u32,
    pub data: Option<Vec<u8>>,
    pub dlc: Option<u8>,
    pub is_error_frame: bool,
    pub timestamp: Option<f64>,
}

fn option_to_str<T: Debug>(o: &Option<T>) -> String {
    if let Some(v) = o {
        format!("{v:X?}")
    } else {
        "None".into()
    }
}

impl Display for PyCanMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = option_to_str(&self.data);
        let dlc = option_to_str(&self.dlc);
        let timestamp = option_to_str(&self.timestamp);

        if self.is_error_frame {
            write!(f, "PyCanMessage: @{timestamp} ERROR FRAME")
        } else {
            write!(
                f,
                "PyCanMessage: @{timestamp} | id=0x{:03X} | dlc={dlc} | data={data}",
                self.arbitration_id
            )
        }
    }
}
