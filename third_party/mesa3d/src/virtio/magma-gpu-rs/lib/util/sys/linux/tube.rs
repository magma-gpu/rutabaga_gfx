// Copyright 2026 Google
// SPDX-License-Identifier: MIT

use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::IoSlice;
use std::io::IoSliceMut;
use std::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::path::Path;

use rustix::cmsg_space;
use rustix::event::poll;
use rustix::event::PollFd;
use rustix::event::PollFlags;
use rustix::fs::fcntl_setfl;
use rustix::fs::OFlags;
use rustix::net::accept;
use rustix::net::bind;
use rustix::net::connect;
use rustix::net::listen;
use rustix::net::recvmsg;
use rustix::net::sendmsg;
use rustix::net::socket_with;
use rustix::net::sockopt::socket_type;
use rustix::net::AddressFamily;
use rustix::net::RecvAncillaryBuffer;
use rustix::net::RecvAncillaryMessage;
use rustix::net::RecvFlags;
use rustix::net::SendAncillaryBuffer;
use rustix::net::SendAncillaryMessage;
use rustix::net::SendFlags;
use rustix::net::SocketAddrUnix;
use rustix::net::SocketFlags;
use rustix::net::SocketType;
use rustix::path::Arg;

use crate::util::AsBorrowedDescriptor;
use crate::util::Error;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;
use crate::util::TubeType;

const MAX_IDENTIFIERS: usize = 28;

pub struct Tube {
    socket: OwnedDescriptor,
}

impl TryFrom<OwnedDescriptor> for Tube {
    type Error = Error;

    fn try_from(socket: OwnedDescriptor) -> Result<Self, Self::Error> {
        let ty = socket_type(&socket)?;
        match ty {
            SocketType::SEQPACKET | SocketType::STREAM => Ok(Tube { socket }),
            _ => Err(Error::Unsupported),
        }
    }
}

impl TryFrom<SocketType> for TubeType {
    type Error = IoError;

    fn try_from(ty: SocketType) -> Result<Self, Self::Error> {
        match ty {
            SocketType::SEQPACKET => Ok(TubeType::Packet),
            SocketType::STREAM => Ok(TubeType::Stream),
            ty => {
                log::warn!("Unsupported socket type {ty:?}");
                Err(IoError::from(ErrorKind::Unsupported))
            }
        }
    }
}

impl Tube {
    pub fn new<P: AsRef<Path> + Arg>(path: P, kind: TubeType) -> MagmaGpuResult<Tube> {
        let socket = match kind {
            TubeType::Packet => socket_with(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::empty(),
                None,
            )?,
            TubeType::Stream => socket_with(
                AddressFamily::UNIX,
                SocketType::STREAM,
                SocketFlags::CLOEXEC,
                None,
            )?,
        };

        let unix_addr = SocketAddrUnix::new(path)?;
        connect(&socket, &unix_addr)?;

        Ok(Tube {
            socket: socket.into(),
        })
    }

    pub fn send(
        &self,
        opaque_data: &[u8],
        descriptors: Vec<OwnedDescriptor>,
    ) -> MagmaGpuResult<usize> {
        let mut total_sent = 0;
        let mut first = true;
        while total_sent < opaque_data.len() {
            let mut space = [MaybeUninit::<u8>::uninit(); cmsg_space!(ScmRights(MAX_IDENTIFIERS))];
            let mut cmsg_buffer = SendAncillaryBuffer::new(&mut space);

            let borrowed_fds: Vec<_> = if first {
                descriptors.iter().map(AsFd::as_fd).collect()
            } else {
                Vec::new()
            };
            if !borrowed_fds.is_empty() {
                let cmsg = SendAncillaryMessage::ScmRights(&borrowed_fds);
                cmsg_buffer.push(cmsg);
            }

            match sendmsg(
                &self.socket,
                &[IoSlice::new(&opaque_data[total_sent..])],
                &mut cmsg_buffer,
                SendFlags::NOSIGNAL,
            ) {
                Ok(n) => {
                    if n == 0 {
                        return Err(Error::IoError(IoError::from(ErrorKind::WriteZero)));
                    }
                    total_sent += n;
                    first = false;
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(rustix::io::Errno::AGAIN) => {
                    match poll(&mut [PollFd::new(&self.socket, PollFlags::OUT)], None) {
                        Ok(_) | Err(rustix::io::Errno::INTR) => continue,
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(total_sent)
    }

    pub fn receive(&self, opaque_data: &mut [u8]) -> MagmaGpuResult<(usize, Vec<OwnedDescriptor>)> {
        loop {
            let mut iovecs = [IoSliceMut::new(opaque_data)];

            let mut space = [MaybeUninit::<u8>::uninit(); cmsg_space!(ScmRights(MAX_IDENTIFIERS))];
            let mut cmsg_buffer = RecvAncillaryBuffer::new(&mut space);
            match recvmsg(
                &self.socket,
                &mut iovecs,
                &mut cmsg_buffer,
                RecvFlags::empty(),
            ) {
                Ok(r) => {
                    let len = r.bytes;
                    let mut received_descriptors: Vec<OwnedDescriptor> = Vec::new();

                    // Iterate over received control messages
                    for cmsg in cmsg_buffer.drain() {
                        match cmsg {
                            RecvAncillaryMessage::ScmRights(fds) => {
                                received_descriptors.extend(fds.into_iter().map(Into::into));
                            }
                            _ => return Err(Error::Unsupported), // Handle unexpected control messages
                        }
                    }

                    return Ok((len, received_descriptors));
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(rustix::io::Errno::AGAIN) => {
                    match poll(&mut [PollFd::new(&self.socket, PollFlags::IN)], None) {
                        Ok(_) | Err(rustix::io::Errno::INTR) => continue,
                        Err(e) => return Err(e.into()),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl AsBorrowedDescriptor for Tube {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.socket
    }
}

pub struct Listener {
    socket: OwnedDescriptor,
}

impl Listener {
    /// Creates a new `Listener` bound to the given path.
    pub fn bind<P: AsRef<Path> + Arg>(path: P) -> MagmaGpuResult<Listener> {
        let socket = socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::empty(),
            None,
        )?;

        let unix_addr = SocketAddrUnix::new(path)?;
        bind(&socket, &unix_addr)?;
        listen(&socket, 128)?;

        fcntl_setfl(&socket, OFlags::NONBLOCK)?;

        Ok(Listener {
            socket: socket.into(),
        })
    }

    pub fn accept(&self) -> MagmaGpuResult<Tube> {
        let accepted_fd = accept(&self.socket)?;
        let descriptor: OwnedDescriptor = accepted_fd.into();
        Ok(Tube { socket: descriptor })
    }
}

impl AsBorrowedDescriptor for Listener {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.socket
    }
}
