extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod display;
pub mod shell;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    display::init_stdlib(vm)?;
    shell::init_stdlib(vm)?;
    Ok(())
}
