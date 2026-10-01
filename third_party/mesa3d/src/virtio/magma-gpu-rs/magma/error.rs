// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::NulError;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::num::ParseIntError;
use std::num::TryFromIntError;
use std::result::Result as StdResult;
use std::str::Utf8Error;

use magma_gpu::util::Error as MagmaGpuError;
use remain::sorted;
#[cfg(any(target_os = "android", target_os = "linux", target_vendor = "apple"))]
use rustix::io::Errno;
use thiserror::Error;

use crate::protocol::MagmaStatus;

/// An error type based on magma_common_defs.h
#[sorted]
#[derive(Error, Debug)]
pub enum Error {
    #[error("Access Denied")]
    AccessDenied,
    #[error("Bad State")]
    BadState,
    #[error("Connection Lost")]
    ConnectionLost,
    #[error("Context Killed")]
    ContextKilled,
    #[error("Internal Error")]
    InternalError,
    #[error("Invalid Arguments")]
    InvalidArgs,
    #[error("Memory Error")]
    MemoryError,
    #[error("A platform error was returned {0}")]
    PlatformError(MagmaGpuError),
    #[error("Timed out")]
    TimedOut,
    #[error("Unimplemented")]
    Unimplemented,
}

impl From<MagmaGpuError> for Error {
    fn from(e: MagmaGpuError) -> Error {
        Error::PlatformError(e)
    }
}

impl From<TryFromIntError> for Error {
    fn from(e: TryFromIntError) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

impl From<NulError> for Error {
    fn from(e: NulError) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

#[cfg(any(target_os = "android", target_os = "linux", target_vendor = "apple"))]
impl From<Errno> for Error {
    fn from(e: Errno) -> Error {
        Error::PlatformError(MagmaGpuError::from(e))
    }
}

impl From<Error> for MagmaStatus {
    fn from(e: Error) -> MagmaStatus {
        match e {
            Error::AccessDenied => MagmaStatus::AccessDenied,
            Error::BadState => MagmaStatus::InternalError,
            Error::ConnectionLost => MagmaStatus::InternalError,
            Error::ContextKilled => MagmaStatus::ContextKilled,
            Error::InternalError => MagmaStatus::InternalError,
            Error::InvalidArgs => MagmaStatus::InvalidArgs,
            Error::MemoryError => MagmaStatus::MemoryError,
            Error::TimedOut => MagmaStatus::TimedOut,
            Error::Unimplemented => MagmaStatus::Unimplemented,
            Error::PlatformError(me) => match me {
                MagmaGpuError::InvalidMagmaHandle
                | MagmaGpuError::NulError(_)
                | MagmaGpuError::ParseIntError(_)
                | MagmaGpuError::TryFromIntError(_)
                | MagmaGpuError::Utf8Error(_) => MagmaStatus::InvalidArgs,
                MagmaGpuError::IoError(ref io_err) => match io_err.kind() {
                    ErrorKind::InvalidInput | ErrorKind::InvalidData => MagmaStatus::InvalidArgs,
                    ErrorKind::OutOfMemory => MagmaStatus::MemoryError,
                    ErrorKind::TimedOut | ErrorKind::WouldBlock => MagmaStatus::TimedOut,
                    ErrorKind::PermissionDenied => MagmaStatus::AccessDenied,
                    ErrorKind::Unsupported => MagmaStatus::Unimplemented,
                    _ => MagmaStatus::InternalError,
                },
                #[cfg(any(target_os = "android", target_os = "linux", target_vendor = "apple"))]
                MagmaGpuError::RustixError(errno) => match errno {
                    Errno::INVAL => MagmaStatus::InvalidArgs,
                    Errno::NOMEM => MagmaStatus::MemoryError,
                    Errno::TIMEDOUT | Errno::AGAIN | Errno::TIME => MagmaStatus::TimedOut,
                    Errno::ACCESS | Errno::PERM => MagmaStatus::AccessDenied,
                    Errno::NOSYS | Errno::OPNOTSUPP => MagmaStatus::Unimplemented,
                    _ => MagmaStatus::InternalError,
                },
                MagmaGpuError::Unsupported => MagmaStatus::Unimplemented,
                _ => MagmaStatus::InternalError,
            },
        }
    }
}

impl From<Error> for i32 {
    fn from(e: Error) -> i32 {
        MagmaStatus::from(e) as i32
    }
}

pub type Result<T> = StdResult<T, Error>;
