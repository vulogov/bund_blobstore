extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};

pub fn init_adam() -> Result<(), Error> {
    match crate::BUND.get().is_some() {
        true => log::info!("BUND Adam instance already initialized."),
        false => match crate::BUND.set(Bund::new()) {
            Ok(_) => log::debug!("BUND Adam instance succesfully initialized."),
            Err(err) => bail!("Error initializing BUND Adam instance: {}", err),
        },
    }
    Ok(())
}
