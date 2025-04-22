// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Creation failed")]
    CreationFailed,
    #[error("No memory")]
    NoMemory,
    #[error("Wrong proxy type")]
    WrongProxyType,
    #[error("Invalid name")]
    InvalidName,
    #[error("Invalid argument")]
    InvalidArgument,
    #[error("Error code {0}")]
    Other(i32),
    #[error(transparent)]
    SpaError(#[from] spa::utils::result::Error),
}

impl Error {
    /// Create an Error from a raw error code
    pub fn from_raw(code: i32) -> Self {
        if code == 0 {
            return Error::Other(0);
        }

        let abs_code = code.abs();
        match abs_code {
            libc::ENOMEM => Error::NoMemory,
            libc::EINVAL => Error::InvalidArgument,
            _ => Error::Other(code),
        }
    }
}

impl From<i32> for Error {
    fn from(code: i32) -> Self {
        Error::from_raw(code)
    }
}
