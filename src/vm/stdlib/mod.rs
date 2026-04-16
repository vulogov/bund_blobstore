extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod bund;
pub mod conditional;
pub mod console;
pub mod encoding;
pub mod filesystem;
pub mod system;

pub fn init_bund_stdlib(vm: &mut Bund) -> Result<(), Error> {
    console::init_stdlib(vm)?;
    bund::init_stdlib(vm)?;
    filesystem::init_stdlib(vm)?;
    conditional::init_stdlib(vm)?;
    system::init_stdlib(vm)?;
    Ok(())
}
