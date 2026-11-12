// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::marker::PhantomData;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

use crate::util::AsBorrowedDescriptor;
use crate::util::Handle;
use crate::util::OwnedDescriptor;
use crate::util::Reader;
use crate::util::Result;
use crate::util::Tube;
use crate::util::Writer;

const MAX_COMMAND_SIZE: usize = 65536;

/// Implemented by any wire protocol (e.g., `KumquatGpuProtocol`, `BroccoliniProtocol`).
pub trait Protocol: Sized {
    /// Variant returned when the peer closes the connection (0 bytes read).
    fn eof() -> Self;

    /// Decodes a single message from `reader` and pops any attached descriptors.
    fn decode(reader: &mut Reader<'_>, descriptors: &mut VecDeque<OwnedDescriptor>)
        -> Result<Self>;
}

/// Outgoing message wrapper for `Stream::write`.
/// Borrows `&'a [u8]` so callers don't need to allocate a `Vec<u8>` just to send a slice.
pub enum Message<'a, T: IntoBytes + FromBytes + Immutable> {
    Cmd(T),
    CmdWithHandle(T, Handle),
    CmdWithData(T, &'a [u8]),
    CmdWithDataAndHandles(T, &'a [u8], Vec<Handle>),
}

/// Framed message stream over a `Tube`, parameterized by the incoming `Protocol`.
pub struct Stream<P: Protocol> {
    stream: Tube,
    write_buffer: Vec<u8>,
    read_buffer: Vec<u8>,
    _phantom: PhantomData<P>,
}

impl<P: Protocol> Stream<P> {
    pub fn new(stream: Tube) -> Self {
        Self {
            stream,
            // Heap-allocated once so `sizeof(Stream<P>)` is 56 bytes instead of 131 KB on stack.
            write_buffer: vec![0; MAX_COMMAND_SIZE],
            read_buffer: vec![0; MAX_COMMAND_SIZE],
            _phantom: PhantomData,
        }
    }

    pub fn write<T: FromBytes + IntoBytes + Immutable>(
        &mut self,
        msg: Message<'_, T>,
    ) -> Result<()> {
        let mut writer = Writer::new(&mut self.write_buffer);

        let descriptors: Vec<OwnedDescriptor> = match msg {
            Message::Cmd(cmd) => {
                writer.write_obj(cmd)?;
                Vec::new()
            }
            Message::CmdWithHandle(cmd, handle) => {
                writer.write_obj(cmd)?;
                vec![handle.os_handle]
            }
            Message::CmdWithData(cmd, data) => {
                writer.write_obj(cmd)?;
                writer.write_all(data)?;
                Vec::new()
            }
            Message::CmdWithDataAndHandles(cmd, data, handles) => {
                writer.write_obj(cmd)?;
                writer.write_all(data)?;
                handles.into_iter().map(|h| h.os_handle).collect()
            }
        };

        let bytes_written = writer.bytes_written();
        self.stream
            .send(&self.write_buffer[0..bytes_written], descriptors)?;
        Ok(())
    }

    pub fn read(&mut self) -> Result<Vec<P>> {
        let mut vec: Vec<P> = Vec::new();
        let (bytes_read, descriptor_vec) = self.stream.receive(&mut self.read_buffer)?;
        let mut descriptors: VecDeque<OwnedDescriptor> = descriptor_vec.into();

        if bytes_read == 0 {
            vec.push(P::eof());
            return Ok(vec);
        }

        let mut reader = Reader::new(&self.read_buffer[0..bytes_read]);
        while reader.available_bytes() != 0 {
            vec.push(P::decode(&mut reader, &mut descriptors)?);
        }

        Ok(vec)
    }

    pub fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        self.stream.as_borrowed_descriptor()
    }
}
