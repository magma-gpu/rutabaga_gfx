// Copyright 2026 Magma-GPU project
// SPDX-License-Identifier: MIT

//! Protocol definitions for sending host-specific buffer handles (such as Android
//! `AHardwareBuffer` metadata and descriptors) over a cross-domain `Tube` socketpair to a host
//! Wayland bridge.

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

pub const CROSS_DOMAIN_WAYLAND_SEND_AHB: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainSendAhb {
    pub cmd: u32,
    pub metadata_size: u32,
    pub num_fds: u32,
    pub pad: u32,
}
