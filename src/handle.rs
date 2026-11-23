// Copyright 2025 Google
// SPDX-License-Identifier: BSD-3-Clause

use magma_gpu::protocols::broccolini_protocol::BroccoliniHandle;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;

use crate::rutabaga_utils::RutabagaResult;

pub struct AhbInfo {
    pub fds: Vec<OwnedDescriptor>,
    pub metadata: Vec<u8>,
}

impl AhbInfo {
    pub fn try_clone(&self) -> RutabagaResult<AhbInfo> {
        let cloned_fds = self
            .fds
            .iter()
            .map(|fd| fd.try_clone())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| MagmaGpuError::InvalidMagmaHandle)?;

        Ok(AhbInfo {
            fds: cloned_fds,
            metadata: self.metadata.clone(),
        })
    }
}

pub enum RutabagaHandle {
    MagmaGpuHandle(MagmaGpuHandle),
    AhbInfo(AhbInfo),
}

impl From<MagmaGpuHandle> for RutabagaHandle {
    fn from(value: MagmaGpuHandle) -> Self {
        RutabagaHandle::MagmaGpuHandle(value)
    }
}

impl TryFrom<RutabagaHandle> for MagmaGpuHandle {
    type Error = MagmaGpuError;

    fn try_from(handle: RutabagaHandle) -> Result<Self, Self::Error> {
        match handle {
            RutabagaHandle::MagmaGpuHandle(h) => Ok(h),
            _ => Err(MagmaGpuError::InvalidMagmaHandle),
        }
    }
}

impl From<AhbInfo> for RutabagaHandle {
    fn from(value: AhbInfo) -> Self {
        RutabagaHandle::AhbInfo(value)
    }
}

impl TryFrom<RutabagaHandle> for AhbInfo {
    type Error = MagmaGpuError;

    fn try_from(handle: RutabagaHandle) -> Result<Self, Self::Error> {
        match handle {
            RutabagaHandle::AhbInfo(h) => Ok(h),
            _ => Err(MagmaGpuError::InvalidMagmaHandle),
        }
    }
}

impl RutabagaHandle {
    /// Clones the RutabagaHandle, duplicating any underlying file descriptors.
    pub fn try_clone(&self) -> RutabagaResult<RutabagaHandle> {
        match self {
            RutabagaHandle::MagmaGpuHandle(handle) => {
                Ok(RutabagaHandle::MagmaGpuHandle(handle.try_clone()?))
            }
            RutabagaHandle::AhbInfo(info) => Ok(RutabagaHandle::AhbInfo(info.try_clone()?)),
        }
    }

    /// Returns a reference to the inner `MagmaGpuHandle` if this is a `MagmaGpuHandle` variant.
    pub fn as_mesa_handle(&self) -> Option<&MagmaGpuHandle> {
        match self {
            RutabagaHandle::MagmaGpuHandle(handle) => Some(handle),
            _ => None,
        }
    }
}

impl From<RutabagaHandle> for BroccoliniHandle {
    fn from(handle: RutabagaHandle) -> Self {
        match handle {
            RutabagaHandle::MagmaGpuHandle(h) => BroccoliniHandle::Magma(h),
            RutabagaHandle::AhbInfo(AhbInfo { fds, metadata }) => {
                BroccoliniHandle::Ahb { fds, metadata }
            }
        }
    }
}

impl From<BroccoliniHandle> for RutabagaHandle {
    fn from(handle: BroccoliniHandle) -> Self {
        match handle {
            BroccoliniHandle::Magma(h) => RutabagaHandle::MagmaGpuHandle(h),
            BroccoliniHandle::Ahb { fds, metadata } => {
                RutabagaHandle::AhbInfo(AhbInfo { fds, metadata })
            }
        }
    }
}
