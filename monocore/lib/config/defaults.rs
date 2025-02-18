use std::{path::PathBuf, sync::LazyLock, time::Duration};

use crate::utils::MONOCORE_HOME_DIR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The default number of vCPUs to use for the MicroVm.
pub const DEFAULT_NUM_VCPUS: u8 = 1;

/// The default amount of RAM in MiB to use for the MicroVm.
pub const DEFAULT_RAM_MIB: u32 = 1024;

/// Default maximum age for log files (7 days)
pub const DEFAULT_LOG_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Default port for the HTTP server
pub const DEFAULT_SERVER_PORT: u16 = 3456;

/// The path where all monocore global data is stored.
pub static DEFAULT_MONOCORE_HOME: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().unwrap().join(MONOCORE_HOME_DIR));

/// The path where all monocore project data is stored.
pub static DEFAULT_MONOCORE_ENV: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().unwrap().join(MONOCORE_HOME_DIR));
