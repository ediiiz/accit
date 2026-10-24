//! The database stores times as Unix milliseconds.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::StoreError;

pub(crate) fn to_millis(time: SystemTime) -> Result<i64, StoreError> {
    let since_epoch = time.duration_since(UNIX_EPOCH).map_err(|_| StoreError::Time)?;

    i64::try_from(since_epoch.as_millis()).map_err(|_| StoreError::Time)
}

pub(crate) fn from_millis(millis: i64) -> Result<SystemTime, StoreError> {
    let millis = u64::try_from(millis).map_err(|_| StoreError::Time)?;

    UNIX_EPOCH.checked_add(Duration::from_millis(millis)).ok_or(StoreError::Time)
}
