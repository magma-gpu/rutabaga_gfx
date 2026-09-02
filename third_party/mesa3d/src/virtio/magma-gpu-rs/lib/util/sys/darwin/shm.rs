// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::ffi::CStr;
use std::os::fd::AsRawFd;
use std::os::fd::IntoRawFd;
use std::os::unix::io::OwnedFd;

use rustix::fs::ftruncate;
use rustix::fs::Mode;
use rustix::shm;

use crate::util::descriptor::AsRawDescriptor;
use crate::util::descriptor::IntoRawDescriptor;
use crate::util::RawDescriptor;
use crate::util::Result as MagmaGpuResult;

pub struct SharedMemory {
    fd: OwnedFd,
    size: u64,
}

impl SharedMemory {
    pub fn new(debug_name: &CStr, size: u64) -> MagmaGpuResult<SharedMemory> {
        let name = format!("/{}", debug_name.to_string_lossy());

        let fd = shm::open(
            name.as_str(),
            shm::OFlags::CREATE | shm::OFlags::EXCL | shm::OFlags::RDWR,
            Mode::RUSR | Mode::WUSR,
        )?;
        shm::unlink(name.as_str())?;

        ftruncate(&fd, size)?;

        Ok(SharedMemory { fd, size })
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

impl AsRawDescriptor for SharedMemory {
    fn as_raw_descriptor(&self) -> RawDescriptor {
        self.fd.as_raw_fd() as RawDescriptor
    }
}

impl IntoRawDescriptor for SharedMemory {
    fn into_raw_descriptor(self) -> RawDescriptor {
        self.fd.into_raw_fd() as RawDescriptor
    }
}

pub fn page_size() -> MagmaGpuResult<u64> {
    Ok(rustix::param::page_size() as _)
}
