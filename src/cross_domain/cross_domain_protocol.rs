// Copyright 2021 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Hand-written protocol for the cross-domain context type.  Intended to be shared with C/C++
//! components.

#![allow(dead_code)]

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

/// Cross-domain commands (only a maximum of 255 supported)
pub const CROSS_DOMAIN_CMD_INIT: u8 = 1;
pub const CROSS_DOMAIN_CMD_GET_IMAGE_REQUIREMENTS: u8 = 2;
pub const CROSS_DOMAIN_CMD_POLL: u8 = 3;
pub const CROSS_DOMAIN_CMD_SEND: u8 = 4;
pub const CROSS_DOMAIN_CMD_RECEIVE: u8 = 5;
pub const CROSS_DOMAIN_CMD_READ: u8 = 6;
pub const CROSS_DOMAIN_CMD_WRITE: u8 = 7;
pub const CROSS_DOMAIN_CMD_CREATE_ATOMIC_MEMORY_SENTINEL: u8 = 8;
pub const CROSS_DOMAIN_CMD_SIGNAL_ATOMIC_MEMORY_SENTINEL: u8 = 9;
pub const CROSS_DOMAIN_CMD_DESTROY_ATOMIC_MEMORY_SENTINEL: u8 = 10;
pub const CROSS_DOMAIN_CMD_READ_CREATE_EVENT: u8 = 11;
pub const CROSS_DOMAIN_CMD_IMPORT_VIRTIOFS_HANDLE: u8 = 12;
pub const CROSS_DOMAIN_CMD_ASSIGN_SOCKET_UUID: u8 = 13;

/// Channel types (must match rutabaga channel types)
pub const CROSS_DOMAIN_CHANNEL_TYPE_WAYLAND: u32 = 0x0001;
pub const CROSS_DOMAIN_CHANNEL_TYPE_CAMERA: u32 = 0x0002;
pub const CROSS_DOMAIN_CHANNEL_TYPE_PIPEWIRE: u32 = 0x0010;
pub const CROSS_DOMAIN_CHANNEL_TYPE_X11: u32 = 0x0011;
pub const CROSS_DOMAIN_CHANNEL_TYPE_DBUS_SESSION: u32 = 0x0012;
pub const CROSS_DOMAIN_CHANNEL_TYPE_DBUS_SYSTEM: u32 = 0x0013;
pub const CROSS_DOMAIN_CHANNEL_TYPE_INTERNAL_SOCKET: u32 = u32::MAX;

/// The maximum number of identifiers
pub const CROSS_DOMAIN_MAX_IDENTIFIERS: usize = 28;

/// virtgpu memory resource ID.  Also works with non-blob memory resources, despite the name.
pub const CROSS_DOMAIN_ID_TYPE_VIRTGPU_BLOB: u32 = 1;
/// virtgpu synchronization resource id.
pub const CROSS_DOMAIN_ID_TYPE_VIRTGPU_SYNC: u32 = 2;
/// ID for Wayland pipe used for reading.  The reading is done by the guest proxy and the host
/// proxy.  The host sends the write end of the proxied pipe over the host Wayland socket.
pub const CROSS_DOMAIN_ID_TYPE_READ_PIPE: u32 = 3;
/// ID for Wayland pipe used for writing.  The writing is done by the guest and the host proxy.
/// The host receives the write end of the pipe over the host Wayland socket.
pub const CROSS_DOMAIN_ID_TYPE_WRITE_PIPE: u32 = 4;

pub const CROSS_DOMAIN_ID_TYPE_EVENT: u32 = 5;

pub const CROSS_DOMAIN_ID_TYPE_VIRTIO_FS_BLOB: u32 = 6;
pub const CROSS_DOMAIN_ID_TYPE_SOCKET: u32 = 7;

/// No ring
pub const CROSS_DOMAIN_RING_NONE: u32 = 0xffffffff;
/// A ring for metadata queries.
pub const CROSS_DOMAIN_QUERY_RING: u32 = 0;
/// A ring based on this particular context's channel.
pub const CROSS_DOMAIN_CHANNEL_RING: u32 = 1;

/// Read pipe IDs start at this value.
pub const CROSS_DOMAIN_PIPE_READ_START: u32 = 0x80000000;
/// Memory watcher IDs start at this value.
pub const CROSS_DOMAIN_ATOMIC_MEMORY_SENTINEL_START: u32 = 0x40000000;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainCapabilities {
    pub version: u32,
    pub supported_channels: u32,
    pub supports_dmabuf: u32,
    pub supports_external_gpu_memory: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainImageRequirements {
    pub strides: [u32; 4],
    pub offsets: [u32; 4],
    pub modifier: u64,
    pub size: u64,
    pub blob_id: u32,
    pub map_info: u32,
    pub memory_idx: i32,
    pub physical_device_idx: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainHeader {
    pub cmd: u8,
    pub ring_idx: u8,
    pub cmd_size: u16,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainInit {
    pub hdr: CrossDomainHeader,
    pub query_ring_id: u32,
    pub channel_ring_id: u32,
    pub channel_type: u32,
    pub internal_socket_uuid: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainGetImageRequirements {
    pub hdr: CrossDomainHeader,
    pub width: u32,
    pub height: u32,
    pub drm_format: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainSendReceive {
    pub hdr: CrossDomainHeader,
    pub num_identifiers: u32,
    pub opaque_data_size: u32,
    pub identifiers: [u32; CROSS_DOMAIN_MAX_IDENTIFIERS],
    pub identifier_types: [u32; CROSS_DOMAIN_MAX_IDENTIFIERS],
    pub identifier_sizes: [u32; CROSS_DOMAIN_MAX_IDENTIFIERS],
    // Data of size "opaque data size follows"
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainReadWrite {
    pub hdr: CrossDomainHeader,
    pub identifier: u32,
    pub hang_up: u32,
    pub opaque_data_size: u32,
    pub pad: u32,
    // Data of size "opaque data size follows"
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainCreateAtomicMemorySentinel {
    pub hdr: CrossDomainHeader,
    /// VirtioFS filesystem ID - identifies which virtio-fs instance
    pub fs_id: u64,
    /// VirtioFS file handle - identifies the file within the filesystem to map and watch
    pub handle: u64,
    /// Memory watcher ID - unique ID for this watcher, used in signal/destroy commands
    pub id: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainSignalAtomicMemorySentinel {
    pub hdr: CrossDomainHeader,
    pub id: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainDestroyAtomicMemorySentinel {
    pub hdr: CrossDomainHeader,
    pub id: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainCreateEvent {
    pub hdr: CrossDomainHeader,
    pub id: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainImportVirtioFsHandle {
    pub hdr: CrossDomainHeader,
    pub fs_id: u64,
    pub handle: u64,
    pub id: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainAssignSocketUuid {
    pub hdr: CrossDomainHeader,
    pub socket_uuid: [u8; 16],
    pub id: u32,
    pub pad: u32,
}

// This is formally not part of the protocol.  This was for ChromeOS and the ChromeOS LTS rutabaga
// branch has it.

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainInitLegacy {
    hdr: CrossDomainHeader,
    query_ring_id: u32,
    channel_type: u32,
}

impl CrossDomainInitLegacy {
    pub fn upgrade(&self) -> CrossDomainInit {
        CrossDomainInit {
            hdr: self.hdr,
            query_ring_id: self.query_ring_id,
            channel_ring_id: self.query_ring_id,
            channel_type: self.channel_type,
            internal_socket_uuid: [0; 16],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, FromBytes, IntoBytes, Immutable)]
pub struct CrossDomainInitV1 {
    pub hdr: CrossDomainHeader,
    pub query_ring_id: u32,
    pub channel_ring_id: u32,
    pub channel_type: u32,
}

impl CrossDomainInitV1 {
    pub fn upgrade(&self) -> CrossDomainInit {
        CrossDomainInit {
            hdr: self.hdr,
            query_ring_id: self.query_ring_id,
            channel_ring_id: self.channel_ring_id,
            channel_type: self.channel_type,
            internal_socket_uuid: [0; 16],
        }
    }
}
