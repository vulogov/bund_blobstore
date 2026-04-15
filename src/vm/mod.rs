use bundcore::bundcore::Bund;
use std::sync::OnceLock;

pub mod vm;
pub use vm::init_adam;

pub static BUND: OnceLock<Bund> = OnceLock::new();
