// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::mem::align_of;
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::util::sys::darwin::mach::*;
use crate::util::Error as MagmaGpuError;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;

pub const MAX_DESCRIPTORS: usize = 28;

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MessageId {
    Payload = 1,
    Hello = 2,
    Signal = 3,
}

#[repr(C, align(4))]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct MachMsgHeader {
    msgh_bits: u32,
    msgh_size: u32,
    msgh_remote_port: u32,
    msgh_local_port: u32,
    msgh_voucher_port: u32,
    msgh_id: i32,
}

#[repr(C, align(4))]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct MachMsgBody {
    msgh_descriptor_count: u32,
}

#[repr(C, align(4))]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
struct MachMsgPortDescriptor {
    name: u32,
    pad1: u32,
    pad2: u16,
    disposition: u8,
    type_: u8,
}

impl MachMsgPortDescriptor {
    fn new(name: mach_port_t, disposition: mach_msg_type_name_t) -> MachMsgPortDescriptor {
        MachMsgPortDescriptor {
            name,
            pad1: 0,
            pad2: 0,
            disposition: disposition as u8,
            type_: MACH_MSG_PORT_DESCRIPTOR as u8,
        }
    }
}

struct Words {
    words: Vec<u32>,
}

impl Words {
    fn new() -> Words {
        let mut words = Words { words: Vec::new() };
        words.push(MachMsgHeader::default().as_bytes());
        words
    }

    fn zeroed(bytes: usize) -> Words {
        Words {
            words: vec![0; bytes.div_ceil(size_of::<u32>())],
        }
    }

    fn len(&self) -> usize {
        self.words.len() * size_of::<u32>()
    }

    fn bytes(&self) -> &[u8] {
        self.words.as_slice().as_bytes()
    }

    fn push(&mut self, part: &[u8]) {
        let start = self.len();
        let end = (start + part.len()).next_multiple_of(align_of::<mach_msg_header_t>());
        self.words.resize(end / size_of::<u32>(), 0);
        self.words.as_mut_slice().as_mut_bytes()[start..start + part.len()].copy_from_slice(part);
    }

    fn set_header(&mut self, header: &MachMsgHeader) {
        self.words.as_mut_slice().as_mut_bytes()[..size_of::<MachMsgHeader>()]
            .copy_from_slice(header.as_bytes());
    }

    fn as_header(&mut self) -> *mut mach_msg_header_t {
        self.words.as_mut_ptr() as *mut mach_msg_header_t
    }
}

pub struct OutgoingMessage {
    id: MessageId,
    descriptors: Vec<OwnedDescriptor>,
    payload: Vec<u8>,
}

impl OutgoingMessage {
    pub fn new(id: MessageId) -> OutgoingMessage {
        OutgoingMessage {
            id,
            descriptors: Vec::new(),
            payload: Vec::new(),
        }
    }

    pub fn with_descriptors(mut self, descriptors: Vec<OwnedDescriptor>) -> OutgoingMessage {
        self.descriptors = descriptors;
        self
    }

    pub fn with_struct<T: IntoBytes + Immutable>(mut self, value: &T) -> OutgoingMessage {
        self.payload.extend_from_slice(value.as_bytes());
        self
    }

    pub fn with_bytes(mut self, bytes: &[u8]) -> OutgoingMessage {
        self.payload.extend_from_slice(bytes);
        self
    }

    pub fn send(self, remote: &SendRight) -> MagmaGpuResult<()> {
        let count = self.descriptors.len();
        if count > MAX_DESCRIPTORS {
            return Err(MagmaGpuError::WithContext(
                "more rights than one message may carry",
            ));
        }

        let mut fileports: Vec<SendRight> = Vec::new();

        let mut words = Words::new();
        if count > 0 {
            words.push(
                MachMsgBody {
                    msgh_descriptor_count: count as u32,
                }
                .as_bytes(),
            );
        }
        for descriptor in &self.descriptors {
            let (name, disposition) = match descriptor {
                OwnedDescriptor::Fd(fd) => {
                    let mut name: mach_port_t = MACH_PORT_NULL;
                    // SAFETY: `fd` is open across the call and `name` is a
                    // valid out parameter.
                    unsafe { fileport_makeport(fd.as_raw_fd(), &mut name) }.check()?;
                    // SAFETY: the call handed over a right this task owns.
                    fileports.push(unsafe { SendRight::from_raw(name) });
                    (name, MACH_MSG_TYPE_MOVE_SEND)
                }
                OwnedDescriptor::MachSend(right) => (right.as_raw(), MACH_MSG_TYPE_MOVE_SEND),
                OwnedDescriptor::MachReceive(right) => (right.as_raw(), MACH_MSG_TYPE_MOVE_RECEIVE),
            };
            words.push(MachMsgPortDescriptor::new(name, disposition).as_bytes());
        }
        words.push(&self.payload);

        let size = words.len();
        words.set_header(&MachMsgHeader {
            msgh_bits: mach_msgh_bits(MACH_MSG_TYPE_COPY_SEND, 0)
                | if count > 0 { MACH_MSGH_BITS_COMPLEX } else { 0 },
            msgh_size: size as u32,
            msgh_remote_port: remote.as_raw(),
            msgh_local_port: MACH_PORT_NULL,
            msgh_voucher_port: MACH_PORT_NULL,
            msgh_id: self.id as mach_msg_id_t,
        });

        // SAFETY: the buffer holds a complete message of `size` bytes, and
        // every right named in it is owned by this task until the send
        // consumes it.
        unsafe {
            mach_msg(
                words.as_header(),
                MACH_SEND_MSG,
                size as mach_msg_size_t,
                0,
                MACH_PORT_NULL,
                MACH_MSG_TIMEOUT_NONE,
                MACH_PORT_NULL,
            )
        }
        .check()?;

        for right in fileports {
            right.into_raw();
        }
        for descriptor in self.descriptors {
            match descriptor {
                // fileport_makeport did not consume it, so dropping it here
                // is what closes it.
                OwnedDescriptor::Fd(_) => (),
                OwnedDescriptor::MachSend(right) => {
                    right.into_raw();
                }
                OwnedDescriptor::MachReceive(right) => {
                    right.into_raw();
                }
            }
        }
        Ok(())
    }
}

pub enum MessageReceived<T> {
    Success(T, Vec<OwnedDescriptor>, Vec<u8>),
    PeerClosed,
}

pub struct IncomingMessage {
    id: MessageId,
}

impl IncomingMessage {
    pub fn new(id: MessageId) -> IncomingMessage {
        IncomingMessage { id }
    }

    pub fn receive<T: FromBytes>(
        self,
        opaque_data_size: usize,
        port: &ReceiveRight,
    ) -> MagmaGpuResult<MessageReceived<T>> {
        let max_size = size_of::<MachMsgHeader>()
            + size_of::<MachMsgBody>()
            + MAX_DESCRIPTORS * size_of::<MachMsgPortDescriptor>()
            + size_of::<T>()
            + opaque_data_size
            + size_of::<mach_msg_max_trailer_t>();
        let mut words = Words::zeroed(max_size);

        let size = words.len() as mach_msg_size_t;
        // SAFETY: receiving into a buffer of known size on a port this task
        // holds the receive right to.
        unsafe {
            mach_msg(
                words.as_header(),
                MACH_RCV_MSG,
                0,
                size,
                port.as_raw(),
                MACH_MSG_TIMEOUT_NONE,
                MACH_PORT_NULL,
            )
        }
        .check()?;

        let (header, _) = MachMsgHeader::read_from_prefix(words.bytes())
            .map_err(|_| MagmaGpuError::WithContext("message is shorter than a header"))?;

        let message = words
            .bytes()
            .get(size_of::<MachMsgHeader>()..header.msgh_size as usize)
            .ok_or(MagmaGpuError::WithContext(
                "message is longer than the buffer it arrived in",
            ))?;

        let (descriptors, rest) = take_descriptors(&header, message)?;

        if header.msgh_id == MACH_NOTIFY_NO_SENDERS {
            return Ok(MessageReceived::PeerClosed);
        }
        if header.msgh_id != self.id as mach_msg_id_t {
            return Err(MagmaGpuError::WithContext(
                "message is not the one expected",
            ));
        }

        let (payload, trailing) = T::read_from_prefix(rest)
            .map_err(|_| MagmaGpuError::WithContext("message is shorter than its payload"))?;
        Ok(MessageReceived::Success(
            payload,
            descriptors,
            trailing.to_vec(),
        ))
    }
}

fn take_descriptors<'a>(
    header: &MachMsgHeader,
    rest: &'a [u8],
) -> MagmaGpuResult<(Vec<OwnedDescriptor>, &'a [u8])> {
    if header.msgh_bits & MACH_MSGH_BITS_COMPLEX == 0 {
        return Ok((Vec::new(), rest));
    }

    let (body, mut rest) = MachMsgBody::read_from_prefix(rest)
        .map_err(|_| MagmaGpuError::WithContext("complex message has no body"))?;
    let count = body.msgh_descriptor_count as usize;
    if count > MAX_DESCRIPTORS {
        return Err(MagmaGpuError::WithContext(
            "message declares more rights than are allowed",
        ));
    }

    let mut descriptors = Vec::with_capacity(count);
    for _ in 0..count {
        let (descriptor, next) = MachMsgPortDescriptor::read_from_prefix(rest)
            .map_err(|_| MagmaGpuError::WithContext("message has a truncated descriptor"))?;
        rest = next;

        if descriptor.disposition as u32 == MACH_MSG_TYPE_PORT_RECEIVE {
            // SAFETY: the descriptor handed over a right this task now owns.
            let right = unsafe { ReceiveRight::from_raw(descriptor.name) };
            descriptors.push(OwnedDescriptor::MachReceive(right));
            continue;
        }

        // SAFETY: the descriptor handed over a right this task now owns.
        let right = unsafe { SendRight::from_raw(descriptor.name) };
        // SAFETY: `right` names a send right this task owns.
        let fd = unsafe { fileport_makefd(right.as_raw()) };
        if fd < 0 {
            descriptors.push(OwnedDescriptor::MachSend(right));
            continue;
        }

        drop(right);
        // SAFETY: `fileport_makefd` returned a descriptor this task owns.
        descriptors.push(OwnedDescriptor::Fd(unsafe { OwnedFd::from_raw_fd(fd) }));
    }
    Ok((descriptors, rest))
}
