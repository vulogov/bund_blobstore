extern crate log;

use easy_error::{Error, bail};
use std::path::PathBuf;

use crate::{DataDistributionManager, DistributionStrategy};

pub fn init_default_db(path: &str) -> Result<(), Error> {
    let data_dir = PathBuf::from(path);
    if !data_dir.exists() {
        match std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))
        {
            Ok(_) => {}
            Err(err) => bail!("{}", err),
        }
    }

    // Initialize DataDistributionManager
    let manager = match DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin) {
        Ok(manager) => manager,
        Err(err) => bail!("{}", err),
    };
    match crate::DB.get().is_some() {
        true => log::info!("Global DB instance already initialized."),
        false => match crate::DB.set(manager) {
            Ok(_) => {
                log::debug!("BUND Adam instance succesfully initialized.")
            }
            Err(_) => bail!("Error initializing global DB instance"),
        },
    }
    Ok(())
}
