extern crate log;

use bundcore::bundcore::Bund;
use easy_error::Error;

pub mod bund_eval;

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    bund_eval::init_stdlib(vm)?;
    Ok(())
}
