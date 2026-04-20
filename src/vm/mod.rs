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

pub static BUND: OnceLock<RwLock<Bund>> = OnceLock::new();
pub static DB: OnceLock<DataDistributionManager> = OnceLock::new();
