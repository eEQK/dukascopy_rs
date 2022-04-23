#![doc = include_str!("../README.md")]
mod data_supplier;
mod dukascopy_service;
mod error;
mod tick;

pub use data_supplier::DataSupplier;
pub use dukascopy_service::DukascopyService;
pub use error::{Error, Kind};
pub use tick::Tick;
