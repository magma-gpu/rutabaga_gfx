// Copyright 2025 Google
// SPDX-License-Identifier: BSD-3-Clause

use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::OwnedDescriptor;

use crate::rutabaga_utils::RutabagaResult;

pub enum RutabagaHandle {
    MesaHandle(MesaHandle),
    AhbInfo {
        fds: Vec<OwnedDescriptor>,
        metadata: Vec<u8>,
    },
}

impl From<MesaHandle> for RutabagaHandle {
    fn from(value: MesaHandle) -> Self {
        RutabagaHandle::MesaHandle(value)
    }
}

impl TryFrom<RutabagaHandle> for MesaHandle {
    type Error = MesaError;

    fn try_from(handle: RutabagaHandle) -> Result<Self, Self::Error> {
        match handle {
            RutabagaHandle::MesaHandle(h) => Ok(h),
            _ => Err(MesaError::InvalidMesaHandle),
        }
    }
}

impl RutabagaHandle {
    /// Clones the RutabagaHandle, duplicating any underlying file descriptors.
    pub fn try_clone(&self) -> RutabagaResult<RutabagaHandle> {
        match self {
            RutabagaHandle::MesaHandle(handle) => {
                Ok(RutabagaHandle::MesaHandle(handle.try_clone()?))
            }
            RutabagaHandle::AhbInfo { fds, metadata } => {
                let cloned_fds = fds
                    .iter()
                    .map(|fd| fd.try_clone())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| MesaError::InvalidMesaHandle)?;

                Ok(RutabagaHandle::AhbInfo {
                    fds: cloned_fds,
                    metadata: metadata.clone(),
                })
            }
        }
    }

    /// Returns a reference to the inner `MesaHandle` if this is a `MesaHandle` variant.
    pub fn as_mesa_handle(&self) -> Option<&MesaHandle> {
        match self {
            RutabagaHandle::MesaHandle(handle) => Some(handle),
            _ => None,
        }
    }
}
