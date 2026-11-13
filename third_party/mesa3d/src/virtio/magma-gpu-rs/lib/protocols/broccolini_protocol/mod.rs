// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

//! IPC protocol between Rutabaga (`ZygoteChannel` / `RemoteContext`) and `broccolini`.

use std::collections::VecDeque;
use std::mem::size_of;

use crate::protocols::ipc::Protocol;
pub use crate::protocols::kumquat_gpu_protocol::DeviceId;
pub use crate::protocols::kumquat_gpu_protocol::VulkanInfo;
use crate::util::Error;
use crate::util::Handle as MagmaGpuHandle;
use crate::util::OwnedDescriptor;
use crate::util::Reader;
use crate::util::Result;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

// Command opcodes
pub const BROCCOLINI_CMD_CTX_CREATE: u32 = 0x100;
pub const BROCCOLINI_CMD_CTX_DESTROY: u32 = 0x101;
pub const BROCCOLINI_CMD_CTX_ATTACH_RESOURCE: u32 = 0x102;
pub const BROCCOLINI_CMD_CTX_DETACH_RESOURCE: u32 = 0x103;
pub const BROCCOLINI_CMD_RESOURCE_CREATE_BLOB: u32 = 0x104;
pub const BROCCOLINI_CMD_SUBMIT_3D: u32 = 0x105;
pub const BROCCOLINI_CMD_CREATE_FENCE: u32 = 0x106;
pub const BROCCOLINI_CMD_GET_NUM_CAPSETS: u32 = 0x107;
pub const BROCCOLINI_CMD_GET_CAPSET_INFO: u32 = 0x108;
pub const BROCCOLINI_CMD_GET_CAPSET: u32 = 0x109;

// Response opcodes
pub const BROCCOLINI_RESP_NODATA: u32 = 0x200;
pub const BROCCOLINI_RESP_CTX_CREATE_HANDLE: u32 = 0x201;
pub const BROCCOLINI_RESP_RESOURCE_CREATE: u32 = 0x202;
pub const BROCCOLINI_RESP_CREATE_FENCE: u32 = 0x203;
pub const BROCCOLINI_RESP_NUM_CAPSETS: u32 = 0x204;
pub const BROCCOLINI_RESP_CAPSET_INFO: u32 = 0x205;
pub const BROCCOLINI_RESP_CAPSET: u32 = 0x206;

// Bitflags for broccolini_resp_resource_create::flags
pub const BROCCOLINI_RESOURCE_FLAG_MAP_INFO: u32 = 1 << 0;
pub const BROCCOLINI_RESOURCE_FLAG_VULKAN_INFO: u32 = 1 << 1;
pub const BROCCOLINI_RESOURCE_FLAG_3D_INFO: u32 = 1 << 2;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_ctrl_hdr {
    pub type_: u32,
    pub payload: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_get_capset {
    pub hdr: broccolini_ctrl_hdr,
    pub capset_id: u32,
    pub capset_version: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_resp_capset_info {
    pub hdr: broccolini_ctrl_hdr,
    pub capset_id: u32,
    pub version: u32,
    pub size: u32,
    pub padding: u32,
}

/// Sent over Zygote Tube accompanied by `fence_write_pipe` descriptor.
#[repr(C)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_ctx_create {
    pub hdr: broccolini_ctrl_hdr, // payload = ctx_id
    pub nlen: u32,
    pub context_init: u32,
    pub debug_name: [u8; 64],
}

impl Default for broccolini_ctx_create {
    fn default() -> Self {
        // SAFETY: All zero pattern is safe for this particular struct.
        unsafe { ::std::mem::zeroed() }
    }
}

/// Sent over Worker Tube (with optional input handle).
/// Maps to `RutabagaContext::context_create_blob`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_resource_create_blob {
    pub hdr: broccolini_ctrl_hdr, // payload = optional input handle_type
    pub resource_id: u32,
    pub blob_mem: u32,
    pub blob_flags: u32,
    pub padding: u32,
    pub blob_id: u64,
    pub size: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_resp_resource_create {
    pub hdr: broccolini_ctrl_hdr,
    pub resource_id: u32,
    pub handle_type: u32,
    pub flags: u32,
    pub map_info: u32,
    pub vulkan_info: VulkanInfo,
    pub width: u32,
    pub height: u32,
    pub drm_fourcc: u32,
    pub strides: [u32; 4],
    pub offsets: [u32; 4],
    pub modifier: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_ctx_attach_resource {
    pub hdr: broccolini_ctrl_hdr,
    pub resource_id: u32,
    pub handle_type: u32,
    pub flags: u32,
    pub width: u32,
    pub height: u32,
    pub drm_fourcc: u32,
    pub strides: [u32; 4],
    pub offsets: [u32; 4],
    pub modifier: u64,
    pub size: u64,
    pub vulkan_info: VulkanInfo,
    pub padding: u32,
}

/// Followed by `[u64; num_in_fences]` + `[u8; cmd_size]`, and accompanied
/// by `hdr.payload` descriptors (`shareable_fences: Vec<MagmaGpuHandle>`).
/// Maps to `RutabagaContext::submit_cmd`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_cmd_submit_3d {
    pub hdr: broccolini_ctrl_hdr,
    pub cmd_size: u32,
    pub num_in_fences: u32,
    pub in_fence_handle_types: [u32; 4],
}

/// Maps to `RutabagaContext::context_create_fence`.
/// Accompanied by optional `EventSignaler` handle.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_create_fence {
    pub hdr: broccolini_ctrl_hdr,
    pub flags: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3],
    pub fence_id: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
pub struct broccolini_resp_create_fence {
    pub hdr: broccolini_ctrl_hdr, // payload = handle_type
    pub fence_id: u64,
}

#[derive(Debug)]
pub enum BroccoliniProtocol {
    // Zygote messages
    GetNumCapsets,
    RespNumCapsets(u32),
    GetCapsetInfo(u32),
    RespCapsetInfo(broccolini_resp_capset_info),
    GetCapset(broccolini_get_capset),
    RespCapset(Vec<u8>),
    CtxCreate(
        broccolini_ctx_create,
        OwnedDescriptor, /* fence_write_pipe */
    ),
    RespCtxCreateHandle(
        u32,             /* ctx_id */
        OwnedDescriptor, /* worker_tube */
    ),

    // Worker messages
    ResourceCreateBlob(broccolini_resource_create_blob, Option<MagmaGpuHandle>),
    RespResourceCreate(broccolini_resp_resource_create, MagmaGpuHandle),
    CtxAttachResource(broccolini_ctx_attach_resource, MagmaGpuHandle),
    CtxDetachResource(u32 /* resource_id */),
    CmdSubmit3d {
        cmd_buf: Vec<u8>,
        fence_ids: Vec<u64>,
        shareable_fences: Vec<MagmaGpuHandle>,
    },
    CreateFence(broccolini_create_fence, Option<MagmaGpuHandle>),
    RespCreateFence(u64 /* fence_id */, MagmaGpuHandle),
    CtxDestroy,
    OkNoData,
    Terminate,
}

impl Protocol for BroccoliniProtocol {
    fn eof() -> Self {
        BroccoliniProtocol::Terminate
    }

    fn decode(
        reader: &mut Reader<'_>,
        descriptors: &mut VecDeque<OwnedDescriptor>,
    ) -> Result<Self> {
        let hdr = reader.peek_obj::<broccolini_ctrl_hdr>()?;
        let protocol = match hdr.type_ {
            BROCCOLINI_CMD_GET_NUM_CAPSETS => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::GetNumCapsets
            }
            BROCCOLINI_RESP_NUM_CAPSETS => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::RespNumCapsets(hdr.payload)
            }
            BROCCOLINI_CMD_GET_CAPSET_INFO => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::GetCapsetInfo(hdr.payload)
            }
            BROCCOLINI_RESP_CAPSET_INFO => BroccoliniProtocol::RespCapsetInfo(reader.read_obj()?),
            BROCCOLINI_CMD_GET_CAPSET => BroccoliniProtocol::GetCapset(reader.read_obj()?),
            BROCCOLINI_RESP_CAPSET => {
                let len: usize = hdr.payload.try_into()?;
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                let mut capset: Vec<u8> = vec![0; len];
                reader.read_exact(&mut capset)?;
                BroccoliniProtocol::RespCapset(capset)
            }
            BROCCOLINI_CMD_CTX_CREATE => {
                let req: broccolini_ctx_create = reader.read_obj()?;
                let fence_write_pipe = descriptors.pop_front().ok_or(Error::Unsupported)?;
                BroccoliniProtocol::CtxCreate(req, fence_write_pipe)
            }
            BROCCOLINI_RESP_CTX_CREATE_HANDLE => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                let worker_tube = descriptors.pop_front().ok_or(Error::Unsupported)?;
                BroccoliniProtocol::RespCtxCreateHandle(hdr.payload, worker_tube)
            }
            BROCCOLINI_CMD_RESOURCE_CREATE_BLOB => {
                let req: broccolini_resource_create_blob = reader.read_obj()?;
                let handle_opt = if req.hdr.payload != 0 {
                    let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                    Some(MagmaGpuHandle {
                        os_handle,
                        handle_type: req.hdr.payload,
                    })
                } else {
                    None
                };
                BroccoliniProtocol::ResourceCreateBlob(req, handle_opt)
            }
            BROCCOLINI_RESP_RESOURCE_CREATE => {
                let resp: broccolini_resp_resource_create = reader.read_obj()?;
                let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                let handle = MagmaGpuHandle {
                    os_handle,
                    handle_type: resp.handle_type,
                };
                BroccoliniProtocol::RespResourceCreate(resp, handle)
            }
            BROCCOLINI_CMD_CTX_ATTACH_RESOURCE => {
                let req: broccolini_ctx_attach_resource = reader.read_obj()?;
                let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                let handle = MagmaGpuHandle {
                    os_handle,
                    handle_type: req.handle_type,
                };
                BroccoliniProtocol::CtxAttachResource(req, handle)
            }
            BROCCOLINI_CMD_CTX_DETACH_RESOURCE => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::CtxDetachResource(hdr.payload)
            }
            BROCCOLINI_CMD_SUBMIT_3D => {
                let cmd: broccolini_cmd_submit_3d = reader.read_obj()?;
                let num_in_fences: usize = cmd.num_in_fences.try_into()?;
                let cmd_size: usize = cmd.cmd_size.try_into()?;
                let num_shareable_fences: usize = cmd.hdr.payload.try_into()?;
                if num_shareable_fences > 4 {
                    return Err(Error::Unsupported);
                }
                let needed_bytes = num_in_fences
                    .checked_mul(size_of::<u64>())
                    .and_then(|n| n.checked_add(cmd_size))
                    .ok_or(Error::Unsupported)?;
                if reader.available_bytes() < needed_bytes {
                    return Err(Error::Unsupported);
                }
                let mut fence_ids = Vec::with_capacity(num_in_fences);
                for _ in 0..num_in_fences {
                    fence_ids.push(reader.read_obj::<u64>()?);
                }
                let mut cmd_buf = vec![0u8; cmd_size];
                if cmd_size > 0 {
                    reader.read_exact(&mut cmd_buf)?;
                }
                let mut shareable_fences = Vec::with_capacity(num_shareable_fences);
                for i in 0..num_shareable_fences {
                    let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                    shareable_fences.push(MagmaGpuHandle {
                        os_handle,
                        handle_type: cmd.in_fence_handle_types[i],
                    });
                }
                BroccoliniProtocol::CmdSubmit3d {
                    cmd_buf,
                    fence_ids,
                    shareable_fences,
                }
            }
            BROCCOLINI_CMD_CREATE_FENCE => {
                let req: broccolini_create_fence = reader.read_obj()?;
                let handle_opt = if req.hdr.payload != 0 {
                    let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                    Some(MagmaGpuHandle {
                        os_handle,
                        handle_type: req.hdr.payload,
                    })
                } else {
                    None
                };
                BroccoliniProtocol::CreateFence(req, handle_opt)
            }
            BROCCOLINI_RESP_CREATE_FENCE => {
                let resp: broccolini_resp_create_fence = reader.read_obj()?;
                let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
                let handle = MagmaGpuHandle {
                    os_handle,
                    handle_type: resp.hdr.payload,
                };
                BroccoliniProtocol::RespCreateFence(resp.fence_id, handle)
            }
            BROCCOLINI_CMD_CTX_DESTROY => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::CtxDestroy
            }
            BROCCOLINI_RESP_NODATA => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::OkNoData
            }
            _ => {
                return Err(Error::Unsupported);
            }
        };

        Ok(protocol)
    }
}
