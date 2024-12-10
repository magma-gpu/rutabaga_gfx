// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use mesa3d_util::MesaHandle;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

#[repr(C)]
pub struct DeviceId {
    pub device_uuid: [u8; 16],
    pub driver_uuid: [u8; 16],
}

/// Memory index and physical device id of the associated VkDeviceMemory.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    FromBytes,
    IntoBytes,
    Immutable,
)]
#[repr(C)]
pub struct VulkanInfo {
    pub memory_idx: u32,
    pub device_id: DeviceId,
}

pub const MAGMA_VIRTIO_GET_CAPABILITIES: u32 = 0x100;

#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct magma_virtio_ctrl_hdr {
    pub type_: u32,
    pub payload: u32,
}

/* KUMQUAT_GPU_PROTOCOL_TRANSFER_TO_HOST_3D, KUMQUAT_GPU_PROTOCOL_TRANSFER_FROM_HOST_3D */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_transfer_host_3d {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub box_: kumquat_gpu_protocol_box,
    pub offset: u64,
    pub level: u32,
    pub stride: u32,
    pub layer_stride: u32,
    pub ctx_id: u32,
    pub resource_id: u32,
    pub padding: u32,
}

/* KUMQUAT_GPU_PROTOCOL_RESOURCE_CREATE_3D */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_resource_create_3d {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub target: u32,
    pub format: u32,
    pub bind: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub array_size: u32,
    pub last_level: u32,
    pub nr_samples: u32,
    pub flags: u32,
    pub size: u32,
    pub stride: u32,
    pub ctx_id: u32,
}

#[derive(Clone, Debug, Copy, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_ctx_create {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub nlen: u32,
    pub context_init: u32,
    pub debug_name: [u8; 64],
}

impl Default for kumquat_gpu_protocol_ctx_create {
    fn default() -> Self {
        // SAFETY: All zero pattern is safe for this particular struct
        unsafe { ::std::mem::zeroed() }
    }
}

/* KUMQUAT_GPU_PROTOCOL_CTX_ATTACH_RESOURCE, KUMQUAT_GPU_PROTOCOL_CTX_DETACH_RESOURCE */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_ctx_resource {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub ctx_id: u32,
    pub resource_id: u32,
}

/* KUMQUAT_GPU_PROTOCOL_SUBMIT_3D */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_cmd_submit {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub ctx_id: u32,
    pub pad: u32,
    pub size: u32,

    // The in-fence IDs are prepended to the cmd_buf and memory layout
    // of the KUMQUAT_GPU_PROTOCOL_SUBMIT_3D buffer looks like this:
    //   _________________
    //   | CMD_SUBMIT_3D |
    //   -----------------
    //   |  header       |
    //   |  in-fence IDs |
    //   |  cmd_buf      |
    //   -----------------
    //
    // This makes in-fence IDs naturally aligned to the sizeof(u64) inside
    // of the virtio buffer.
    pub num_in_fences: u32,
    pub flags: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3],
}

/* KUMQUAT_GPU_PROTOCOL_RESP_CAPSET_INFO */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_resp_capset_info {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub capset_id: u32,
    pub version: u32,
    pub size: u32,
    pub padding: u32,
}

/* KUMQUAT_GPU_PROTOCOL_GET_CAPSET */
#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_get_capset {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub capset_id: u32,
    pub capset_version: u32,
}

#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_resource_create_blob {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub ctx_id: u32,
    pub blob_mem: u32,
    pub blob_flags: u32,
    pub padding: u32,
    pub blob_id: u64,
    pub size: u64,
}

#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_resp_resource_create {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub resource_id: u32,
    pub handle_type: u32,
    pub vulkan_info: VulkanInfo,
}

#[derive(Copy, Clone, Debug, Default, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct kumquat_gpu_protocol_resp_cmd_submit_3d {
    pub hdr: kumquat_gpu_protocol_ctrl_hdr,
    pub fence_id: u64,
    pub handle_type: u32,
    pub padding: u32,
}

/// A virtio gpu command and associated metadata specific to each command.
#[derive(Debug)]
pub enum KumquatGpuProtocol {
    OkNoData,
    GetNumCapsets,
    GetCapsetInfo(u32),
    GetCapset(kumquat_gpu_protocol_get_capset),
    CtxCreate(kumquat_gpu_protocol_ctx_create),
    CtxDestroy(u32),
    CtxAttachResource(kumquat_gpu_protocol_ctx_resource),
    CtxDetachResource(kumquat_gpu_protocol_ctx_resource),
    ResourceCreate3d(kumquat_gpu_protocol_resource_create_3d),
    TransferToHost3d(kumquat_gpu_protocol_transfer_host_3d, MesaHandle),
    TransferFromHost3d(kumquat_gpu_protocol_transfer_host_3d, MesaHandle),
    CmdSubmit3d(kumquat_gpu_protocol_cmd_submit, Vec<u8>, Vec<u64>),
    ResourceCreateBlob(kumquat_gpu_protocol_resource_create_blob),
    SnapshotSave,
    SnapshotRestore,
    RespNumCapsets(u32),
    RespCapsetInfo(kumquat_gpu_protocol_resp_capset_info),
    RespCapset(Vec<u8>),
    RespContextCreate(u32),
    RespResourceCreate(kumquat_gpu_protocol_resp_resource_create, MesaHandle),
    RespCmdSubmit3d(u64, MesaHandle),
    RespOkSnapshot,
}

pub enum KumquatGpuProtocolWrite<T: IntoBytes + FromBytes + Immutable> {
    Cmd(T),
    CmdWithHandle(T, MesaHandle),
    CmdWithData(T, Vec<u8>),
}
