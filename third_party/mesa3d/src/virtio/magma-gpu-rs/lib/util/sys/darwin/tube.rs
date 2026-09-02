// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::io::Error;
use std::io::ErrorKind;
use std::os::fd::AsFd;
use std::os::fd::BorrowedFd;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use rustix::io::fcntl_setfd;
use rustix::io::read;
use rustix::io::write;
use rustix::io::FdFlags;
use rustix::net::accept;
use rustix::net::bind;
use rustix::net::connect;
use rustix::net::listen;
use rustix::net::socket_with;
use rustix::net::AddressFamily;
use rustix::net::SocketAddrUnix;
use rustix::net::SocketFlags;
use rustix::net::SocketType;
use rustix::path::Arg;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::util::sys::darwin::mach::*;
use crate::util::sys::darwin::message::IncomingMessage;
use crate::util::sys::darwin::message::MessageId;
use crate::util::sys::darwin::message::MessageReceived;
use crate::util::sys::darwin::message::OutgoingMessage;
use crate::util::AsBorrowedDescriptor;
use crate::util::Error as MagmaGpuError;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;
use crate::util::TubeType;

const MAX_PAYLOAD_SIZE: usize = 8192;

const RENDEZVOUS_NAME_SIZE: usize = 128;

impl TryFrom<SocketType> for TubeType {
    type Error = Error;

    fn try_from(ty: SocketType) -> Result<Self, Error> {
        match ty {
            SocketType::STREAM => Ok(TubeType::Packet),
            ty => {
                log::warn!("Unsupported socket type {ty:?}");
                Err(Error::from(ErrorKind::Unsupported))
            }
        }
    }
}

#[repr(C, align(4))]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct TubePayloadHeader {
    len: u32,
}

static RENDEZVOUS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn rendezvous_name(path: &str) -> MagmaGpuResult<CString> {
    let mut name = String::from("org.mesa3d.magma");
    for part in path.split('/').filter(|part| !part.is_empty()) {
        name.push('.');
        name.push_str(part);
    }
    let count = RENDEZVOUS_COUNTER.fetch_add(1, Ordering::Relaxed);
    name.push_str(&format!(".{}.{}", std::process::id(), count));
    CString::new(name).map_err(MagmaGpuError::from)
}

fn write_name(socket: BorrowedFd<'_>, name: &CString) -> MagmaGpuResult<()> {
    let bytes = name.as_bytes_with_nul();
    if bytes.len() > RENDEZVOUS_NAME_SIZE {
        return Err(MagmaGpuError::WithContext("rendezvous name is too long"));
    }
    let mut frame = [0u8; RENDEZVOUS_NAME_SIZE];
    frame[..bytes.len()].copy_from_slice(bytes);

    let mut written = 0;
    while written < frame.len() {
        match write(socket, &frame[written..])? {
            0 => return Err(MagmaGpuError::WithContext("peer closed during rendezvous")),
            n => written += n,
        }
    }
    Ok(())
}

fn read_name(socket: BorrowedFd<'_>) -> MagmaGpuResult<CString> {
    let mut frame = [0u8; RENDEZVOUS_NAME_SIZE];
    let mut filled = 0;
    while filled < frame.len() {
        match read(socket, &mut frame[filled..])? {
            0 => return Err(MagmaGpuError::WithContext("peer closed during rendezvous")),
            n => filled += n,
        }
    }

    let end = frame
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(MagmaGpuError::WithContext(
            "rendezvous name is not terminated",
        ))?;
    CString::new(&frame[..end]).map_err(MagmaGpuError::from)
}

fn check_in(name: &CString) -> MagmaGpuResult<ReceiveRight> {
    let mut port: mach_port_t = MACH_PORT_NULL;
    // SAFETY: `name` outlives the call, and `port` is a valid out parameter.
    unsafe { bootstrap_check_in(bootstrap_port, name.as_ptr(), &mut port) }.check()?;
    // SAFETY: the call handed over a receive right this task now owns.
    Ok(unsafe { ReceiveRight::from_raw(port) })
}

fn look_up(name: &CString) -> MagmaGpuResult<SendRight> {
    let mut port: mach_port_t = MACH_PORT_NULL;
    // SAFETY: `name` outlives the call, and `port` is a valid out parameter.
    unsafe { bootstrap_look_up(bootstrap_port, name.as_ptr(), &mut port) }.check()?;
    // SAFETY: the lookup handed over a send right this task now owns.
    Ok(unsafe { SendRight::from_raw(port) })
}

fn send_hello(peer: &SendRight, port: &ReceiveRight) -> MagmaGpuResult<()> {
    OutgoingMessage::new(MessageId::Hello)
        .with_descriptors(vec![port.make_send()?.into()])
        .send(peer)
}

fn accept_hello(port: &ReceiveRight) -> MagmaGpuResult<SendRight> {
    let hello = IncomingMessage::new(MessageId::Hello).receive::<()>(0, port)?;
    let MessageReceived::Success(_, descriptors, _) = hello else {
        return Err(MagmaGpuError::WithContext("peer did not introduce itself"));
    };

    match descriptors.into_iter().next() {
        Some(OwnedDescriptor::MachSend(right)) => Ok(right),
        _ => Err(MagmaGpuError::WithContext("peer sent no reply right")),
    }
}

pub struct Tube {
    receive: OwnedDescriptor,
    peer: SendRight,
}

impl Tube {
    pub fn new<P: AsRef<Path> + Arg>(path: P, kind: TubeType) -> MagmaGpuResult<Tube> {
        if !matches!(kind, TubeType::Packet) {
            return Err(MagmaGpuError::Unsupported);
        }

        let socket = socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::empty(),
            None,
        )?;
        fcntl_setfd(&socket, FdFlags::CLOEXEC)?;
        let unix_addr = SocketAddrUnix::new(path)?;
        connect(&socket, &unix_addr)?;

        let introduction = look_up(&read_name(socket.as_fd())?)?;

        let receive = ReceiveRight::new()?;

        send_hello(&introduction, &receive)?;
        drop(introduction);

        receive.request_no_senders()?;
        let peer = accept_hello(&receive)?;

        Ok(Tube {
            receive: receive.into(),
            peer,
        })
    }

    pub fn send(
        &self,
        opaque_data: &[u8],
        descriptors: Vec<OwnedDescriptor>,
    ) -> MagmaGpuResult<usize> {
        let payload_len = opaque_data.len();
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(MagmaGpuError::WithContext(
                "message exceeds the maximum size",
            ));
        }

        OutgoingMessage::new(MessageId::Payload)
            .with_descriptors(descriptors)
            .with_struct(&TubePayloadHeader {
                len: payload_len as u32,
            })
            .with_bytes(opaque_data)
            .send(&self.peer)?;

        Ok(payload_len)
    }

    pub fn receive(&self, opaque_data: &mut [u8]) -> MagmaGpuResult<(usize, Vec<OwnedDescriptor>)> {
        let port = self.receive_right()?;

        let message = IncomingMessage::new(MessageId::Payload)
            .receive::<TubePayloadHeader>(MAX_PAYLOAD_SIZE, port)?;

        let MessageReceived::Success(header, descriptors, payload) = message else {
            return Ok((0, Vec::new()));
        };

        let payload_len = header.len as usize;
        if payload_len > opaque_data.len() || payload_len > payload.len() {
            return Err(MagmaGpuError::WithContext(
                "received message exceeds buffer",
            ));
        }
        opaque_data[..payload_len].copy_from_slice(&payload[..payload_len]);

        Ok((payload_len, descriptors))
    }

    fn receive_right(&self) -> MagmaGpuResult<&ReceiveRight> {
        match &self.receive {
            OwnedDescriptor::MachReceive(right) => Ok(right),
            _ => Err(MagmaGpuError::WithContext("a tube holds a receive right")),
        }
    }
}

impl AsBorrowedDescriptor for Tube {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.receive
    }
}

impl TryFrom<OwnedDescriptor> for Tube {
    type Error = MagmaGpuError;

    fn try_from(_descriptor: OwnedDescriptor) -> Result<Self, Self::Error> {
        Err(MagmaGpuError::Unsupported)
    }
}

pub struct Listener {
    socket: OwnedDescriptor,
    path: String,
}

impl Listener {
    pub fn bind<P: AsRef<Path> + Arg>(path: P) -> MagmaGpuResult<Listener> {
        let socket = socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::empty(),
            None,
        )?;
        fcntl_setfd(&socket, FdFlags::CLOEXEC)?;

        let name = path.as_ref().to_string_lossy().into_owned();
        let unix_addr = SocketAddrUnix::new(path)?;
        bind(&socket, &unix_addr)?;
        listen(&socket, 128)?;

        Ok(Listener {
            socket: socket.into(),
            path: name,
        })
    }

    pub fn accept(&self) -> MagmaGpuResult<Tube> {
        let listening = match &self.socket {
            OwnedDescriptor::Fd(fd) => fd.as_fd(),
            _ => return Err(MagmaGpuError::WithContext("a listener holds a socket")),
        };

        let connection = accept(listening)?;

        let name = rendezvous_name(&self.path)?;
        let introduction = check_in(&name)?;
        write_name(connection.as_fd(), &name)?;

        let peer = accept_hello(&introduction)?;
        drop(introduction);

        let receive = ReceiveRight::new()?;
        send_hello(&peer, &receive)?;
        receive.request_no_senders()?;

        drop(connection);

        Ok(Tube {
            receive: receive.into(),
            peer,
        })
    }
}

impl AsBorrowedDescriptor for Listener {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.socket
    }
}
