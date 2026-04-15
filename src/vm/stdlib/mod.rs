extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod console;

pub fn init_bund_stdlib(vm: &mut Bund) -> Result<(), Error> {
    console::init_stdlib(vm)?;
    Ok(())
}
