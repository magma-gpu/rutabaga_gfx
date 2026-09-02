// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::os::fd::AsFd;

use rustix::io::read;
use rustix::io::write;
use rustix::pipe::pipe;

use crate::util::AsBorrowedDescriptor;
use crate::util::AsRawDescriptor;
use crate::util::Error;
use crate::util::OwnedDescriptor;
use crate::util::RawDescriptor;
use crate::util::Result as MagmaGpuResult;

pub struct ReadPipe {
    descriptor: OwnedDescriptor,
}

pub struct WritePipe {
    descriptor: OwnedDescriptor,
}

pub fn create_pipe() -> MagmaGpuResult<(ReadPipe, WritePipe)> {
    let (read_pipe, write_pipe) = pipe()?;
    Ok((
        ReadPipe {
            descriptor: read_pipe.into(),
        },
        WritePipe {
            descriptor: write_pipe.into(),
        },
    ))
}

impl ReadPipe {
    pub fn read(&self, data: &mut [u8]) -> MagmaGpuResult<usize> {
        let fd = match &self.descriptor {
            OwnedDescriptor::Fd(fd) => fd.as_fd(),
            _ => return Err(Error::WithContext("cannot read a non-file descriptor")),
        };

        let bytes_read = read(fd, data)?;
        Ok(bytes_read)
    }
}

impl AsBorrowedDescriptor for ReadPipe {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}

impl WritePipe {
    pub fn new(descriptor: OwnedDescriptor) -> WritePipe {
        WritePipe { descriptor }
    }

    pub fn write(&self, data: &[u8]) -> MagmaGpuResult<usize> {
        let fd = match &self.descriptor {
            OwnedDescriptor::Fd(fd) => fd.as_fd(),
            _ => return Err(Error::WithContext("cannot write a non-file descriptor")),
        };

        let bytes_written = write(fd, data)?;
        Ok(bytes_written)
    }
}

impl AsBorrowedDescriptor for WritePipe {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}

impl AsRawDescriptor for WritePipe {
    fn as_raw_descriptor(&self) -> RawDescriptor {
        self.descriptor.as_raw_descriptor()
    }
}
