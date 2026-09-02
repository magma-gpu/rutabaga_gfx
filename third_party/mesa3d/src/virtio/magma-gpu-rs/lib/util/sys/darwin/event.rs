// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use crate::util::sys::darwin::mach::ReceiveRight;
use crate::util::sys::darwin::message::IncomingMessage;
use crate::util::sys::darwin::message::MessageId;
use crate::util::sys::darwin::message::MessageReceived;
use crate::util::sys::darwin::message::OutgoingMessage;
use crate::util::AsBorrowedDescriptor;
use crate::util::Error;
use crate::util::Handle;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;
use crate::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD;

pub struct EventSignaler {
    descriptor: OwnedDescriptor,
}

pub struct EventWaiter {
    descriptor: OwnedDescriptor,
}

pub fn create_event_pair() -> MagmaGpuResult<(EventSignaler, EventWaiter)> {
    let receive = ReceiveRight::new()?;
    let send = receive.make_send()?;

    Ok((
        EventSignaler {
            descriptor: OwnedDescriptor::MachSend(send),
        },
        EventWaiter {
            descriptor: OwnedDescriptor::MachReceive(receive),
        },
    ))
}

impl EventSignaler {
    pub fn add(&self, value: u64) -> MagmaGpuResult<()> {
        if value == 0 {
            return Ok(());
        }

        let port = match &self.descriptor {
            OwnedDescriptor::MachSend(right) => right,
            _ => return Err(Error::WithContext("an event signaller holds a send right")),
        };

        OutgoingMessage::new(MessageId::Signal)
            .with_struct(&value)
            .send(port)
    }

    pub fn signal(&self) -> MagmaGpuResult<()> {
        self.add(1)
    }
}

impl EventWaiter {
    pub fn try_clone(&self) -> MagmaGpuResult<EventWaiter> {
        Err(Error::Unsupported)
    }

    pub fn signaler(&self) -> MagmaGpuResult<EventSignaler> {
        let right = match &self.descriptor {
            OwnedDescriptor::MachReceive(right) => right,
            _ => return Err(Error::WithContext("an event waiter holds a receive right")),
        };

        Ok(EventSignaler {
            descriptor: right.make_send()?.into(),
        })
    }

    pub fn wait(&self) -> MagmaGpuResult<u64> {
        let port = match &self.descriptor {
            OwnedDescriptor::MachReceive(right) => right,
            _ => return Err(Error::WithContext("an event waiter holds a receive right")),
        };

        let signal = IncomingMessage::new(MessageId::Signal).receive::<u64>(0, port)?;
        let MessageReceived::Success(count, _, _) = signal else {
            return Err(Error::WithContext("event port has no signaller left"));
        };
        Ok(count)
    }
}

impl TryFrom<Handle> for EventSignaler {
    type Error = Error;
    fn try_from(handle: Handle) -> Result<Self, Self::Error> {
        if handle.handle_type != MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD {
            return Err(Error::InvalidMagmaHandle);
        }

        Ok(EventSignaler {
            descriptor: handle.os_handle,
        })
    }
}

impl TryFrom<Handle> for EventWaiter {
    type Error = Error;
    fn try_from(handle: Handle) -> Result<Self, Self::Error> {
        if handle.handle_type != MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD {
            return Err(Error::InvalidMagmaHandle);
        }

        Ok(EventWaiter {
            descriptor: handle.os_handle,
        })
    }
}

impl From<EventSignaler> for Handle {
    fn from(evt: EventSignaler) -> Self {
        Handle {
            os_handle: evt.descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD,
        }
    }
}

impl From<EventWaiter> for Handle {
    fn from(evt: EventWaiter) -> Self {
        Handle {
            os_handle: evt.descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD,
        }
    }
}

impl AsBorrowedDescriptor for EventSignaler {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}

impl AsBorrowedDescriptor for EventWaiter {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}
