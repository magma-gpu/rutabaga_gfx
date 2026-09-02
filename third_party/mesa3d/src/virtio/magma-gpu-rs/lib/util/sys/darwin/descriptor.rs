// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::os::fd::AsFd;
use std::os::fd::OwnedFd;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::IntoRawFd;
use std::os::unix::io::RawFd;

use rustix::fs::fcntl_getfl;
use rustix::fs::fstat;
use rustix::fs::seek;
use rustix::fs::FileType;
use rustix::fs::OFlags;
use rustix::fs::SeekFrom;
use rustix::net::sockopt::socket_type;

use crate::util::sys::darwin::mach::ReceiveRight;
use crate::util::sys::darwin::mach::SendRight;

use crate::util::descriptor::AsRawDescriptor;
use crate::util::descriptor::FromRawDescriptor;
use crate::util::descriptor::IntoRawDescriptor;
use crate::util::DescriptorType;
use crate::util::MAGMA_GPU_HANDLE_TYPE_MEM_SHM;
use crate::util::MAGMA_MAP_ACCESS_READ;
use crate::util::MAGMA_MAP_ACCESS_RW;
use crate::util::MAGMA_MAP_ACCESS_WRITE;

pub type RawDescriptor = i64;
pub const DEFAULT_RAW_DESCRIPTOR: RawDescriptor = -1;

#[derive(Debug)]
pub enum OwnedDescriptor {
    Fd(OwnedFd),
    MachSend(SendRight),
    MachReceive(ReceiveRight),
}

impl OwnedDescriptor {
    pub fn try_clone(&self) -> Result<OwnedDescriptor> {
        match self {
            OwnedDescriptor::Fd(fd) => Ok(OwnedDescriptor::Fd(fd.try_clone()?)),
            OwnedDescriptor::MachSend(right) => Ok(OwnedDescriptor::MachSend(right.try_clone()?)),
            OwnedDescriptor::MachReceive(_) => Err(Error::from(ErrorKind::Unsupported)),
        }
    }

    pub fn determine_type(&self) -> Result<DescriptorType> {
        let owned = match self {
            OwnedDescriptor::MachSend(_) | OwnedDescriptor::MachReceive(_) => {
                return Ok(DescriptorType::Event)
            }
            OwnedDescriptor::Fd(fd) => fd.as_fd(),
        };
        if let Ok(fd_stat) = fstat(owned) {
            match FileType::from_raw_mode(fd_stat.st_mode) {
                FileType::Socket => {
                    let ty = socket_type(owned)?;
                    return Ok(DescriptorType::Socket(ty.try_into()?));
                }
                FileType::Fifo => {
                    let flags = fcntl_getfl(owned)?;
                    if (flags & OFlags::ACCMODE) == OFlags::WRONLY {
                        return Ok(DescriptorType::WritePipe);
                    }
                }
                _ => {}
            }
        }

        match seek(owned, SeekFrom::End(0)) {
            Ok(seek_size) => {
                seek(owned, SeekFrom::Start(0))?;
                let size: u32 = seek_size
                    .try_into()
                    .map_err(|_| Error::from(ErrorKind::Unsupported))?;
                Ok(DescriptorType::Memory(size, MAGMA_GPU_HANDLE_TYPE_MEM_SHM))
            }
            _ => {
                let flags = fcntl_getfl(owned)?;
                match flags & OFlags::ACCMODE {
                    OFlags::WRONLY => Ok(DescriptorType::WritePipe),
                    _ => Err(Error::from(ErrorKind::Unsupported)),
                }
            }
        }
    }

    pub fn determine_map_access_mode(&self) -> Result<u32> {
        let fd = match self {
            OwnedDescriptor::Fd(fd) => fd.as_fd(),
            _ => return Err(Error::from(ErrorKind::Unsupported)),
        };

        let flags = fcntl_getfl(fd)?;
        let access = match flags & OFlags::ACCMODE {
            OFlags::RDONLY => MAGMA_MAP_ACCESS_READ,
            OFlags::WRONLY => MAGMA_MAP_ACCESS_WRITE,
            OFlags::RDWR => MAGMA_MAP_ACCESS_RW,
            _ => return Err(Error::from(ErrorKind::Unsupported)),
        };
        Ok(access)
    }
}

impl AsRawDescriptor for OwnedDescriptor {
    fn as_raw_descriptor(&self) -> RawDescriptor {
        match self {
            OwnedDescriptor::Fd(fd) => fd.as_raw_fd() as RawDescriptor,
            OwnedDescriptor::MachSend(right) => right.as_raw() as RawDescriptor,
            OwnedDescriptor::MachReceive(right) => right.as_raw() as RawDescriptor,
        }
    }
}

impl FromRawDescriptor for OwnedDescriptor {
    // SAFETY:
    // It is caller's responsibility to ensure that the descriptor is valid and
    // stays valid for the lifetime of Self
    unsafe fn from_raw_descriptor(descriptor: RawDescriptor) -> Self {
        OwnedDescriptor::Fd(OwnedFd::from_raw_fd(descriptor as RawFd))
    }
}

impl IntoRawDescriptor for OwnedDescriptor {
    fn into_raw_descriptor(self) -> RawDescriptor {
        match self {
            OwnedDescriptor::Fd(fd) => fd.into_raw_fd() as RawDescriptor,
            OwnedDescriptor::MachSend(right) => right.into_raw() as RawDescriptor,
            OwnedDescriptor::MachReceive(right) => right.into_raw() as RawDescriptor,
        }
    }
}

impl AsRawDescriptor for File {
    fn as_raw_descriptor(&self) -> RawDescriptor {
        self.as_raw_fd() as RawDescriptor
    }
}

impl FromRawDescriptor for File {
    // SAFETY:
    // It is caller's responsibility to ensure that the descriptor is valid and
    // stays valid for the lifetime of Self
    unsafe fn from_raw_descriptor(descriptor: RawDescriptor) -> Self {
        File::from_raw_fd(descriptor as RawFd)
    }
}

impl IntoRawDescriptor for File {
    fn into_raw_descriptor(self) -> RawDescriptor {
        self.into_raw_fd() as RawDescriptor
    }
}

impl From<File> for OwnedDescriptor {
    fn from(f: File) -> OwnedDescriptor {
        OwnedDescriptor::Fd(f.into())
    }
}

impl From<OwnedFd> for OwnedDescriptor {
    fn from(o: OwnedFd) -> OwnedDescriptor {
        OwnedDescriptor::Fd(o)
    }
}

impl From<ReceiveRight> for OwnedDescriptor {
    fn from(right: ReceiveRight) -> OwnedDescriptor {
        OwnedDescriptor::MachReceive(right)
    }
}

impl From<SendRight> for OwnedDescriptor {
    fn from(right: SendRight) -> OwnedDescriptor {
        OwnedDescriptor::MachSend(right)
    }
}
