extern crate xdg;

use std::string::String;
use std::time::Duration;

use crate::item::Item;

pub mod file;
use self::file::FileOutput;
pub mod influx;
use self::influx::InfluxOutput;
pub mod error;
use self::error::*;

pub trait AKOutput {
    fn prepare(&self, items: &Vec<Item>) -> Result<Self, OutputError>
    where
        Self: std::marker::Sized;
    fn write_value(&self, key: &String, time: Duration, value: f64) -> Result<(), OutputError>;
    fn write_raw_value(&self, key: &String, time: Duration, value: &str)
        -> Result<(), OutputError>;
    fn write_raw_value_as_fallback(
        &self,
        key: &String,
        time: Duration,
        value: &str,
    ) -> Result<(), OutputError>;
    fn clean_up(&self) -> Result<(), OutputError>;
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OutputKind {
    File {
        #[serde(flatten)]
        fo: FileOutput,
    },
    InfluxDB {
        #[serde(flatten)]
        io: InfluxOutput,
    },
    // more in the future
}

impl AKOutput for OutputKind {
    fn prepare(&self, items: &Vec<Item>) -> Result<Self, OutputError> {
        match self {
            OutputKind::File { fo } => fo.prepare(items).map(|o| OutputKind::File { fo: o }),
            OutputKind::InfluxDB { io } => {
                io.prepare(items).map(|o| OutputKind::InfluxDB { io: o })
            }
        }
    }

    fn write_value(&self, key: &String, time: Duration, value: f64) -> Result<(), OutputError> {
        match self {
            OutputKind::File { fo } => fo.write_value(key, time, value),
            OutputKind::InfluxDB { io } => io.write_value(key, time, value),
        }
    }

    fn write_raw_value_as_fallback(
        &self,
        key: &String,
        time: Duration,
        value: &str,
    ) -> Result<(), OutputError> {
        match self {
            OutputKind::File { fo } => fo.write_raw_value_as_fallback(key, time, value),
            OutputKind::InfluxDB { io } => io.write_raw_value_as_fallback(key, time, value),
        }
    }

    fn write_raw_value(
        &self,
        key: &String,
        time: Duration,
        value: &str,
    ) -> Result<(), OutputError> {
        match self {
            OutputKind::File { fo } => fo.write_raw_value(key, time, value),
            OutputKind::InfluxDB { io } => io.write_raw_value(key, time, value),
        }
    }

    fn clean_up(&self) -> Result<(), OutputError> {
        match self {
            OutputKind::File { fo } => fo.clean_up(),
            OutputKind::InfluxDB { io } => io.clean_up(),
        }
    }
}
