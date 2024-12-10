// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#[macro_export]
macro_rules! check_ntstatus {
    ($x: expr) => {{
        match $x {
            windows_sys::Win32::Foundation::STATUS_SUCCESS => Ok(()),
            e => {
                let error = rustix::io::Errno::from_raw_os_error(e);
                Err(MesaError::RustixError(error))
            }
        }
    }};
}

#[macro_export]
macro_rules! log_ntstatus {
    ($x: expr) => {{
        match $x {
            windows_sys::Win32::Foundation::STATUS_SUCCESS => (),
            e => error!("logging error status: {:#X}", e),
        }
    }};
}
