use bundcore::bundcore::Bund;
use parking_lot::RwLock;
use std::sync::OnceLock;

pub mod vm;
pub use vm::init_adam;

use crate::DataDistributionManager;

pub mod eval;
pub mod stdlib;

pub mod helpers;

pub mod db;
pub mod log_provider;
pub use log_provider::LogProvider;

pub static BUND: OnceLock<RwLock<Bund>> = OnceLock::new();
pub static DB: OnceLock<DataDistributionManager> = OnceLock::new();
