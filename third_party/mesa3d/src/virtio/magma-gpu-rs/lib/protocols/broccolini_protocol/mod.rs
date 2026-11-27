// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

//! IPC protocol between Rutabaga (`ZygoteChannel` / `RemoteContext`) and `broccolini`.

use std::collections::VecDeque;
use std::mem::size_of;

use crate::protocols::ipc::Message;
use crate::protocols::ipc::Protocol;
use crate::protocols::ipc::Stream;
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
pub const BROCCOLINI_CMD_GET_CAPSET_INFO: u32 = 0x107;
pub const BROCCOLINI_CMD_GET_CAPSET: u32 = 0x108;

// Response opcodes
pub const BROCCOLINI_RESP_NODATA: u32 = 0x200;
pub const BROCCOLINI_RESP_CTX_CREATE_HANDLE: u32 = 0x201;
pub const BROCCOLINI_RESP_RESOURCE_CREATE: u32 = 0x202;
pub const BROCCOLINI_RESP_CREATE_FENCE: u32 = 0x203;
pub const BROCCOLINI_RESP_CAPSET_INFO: u32 = 0x204;
pub const BROCCOLINI_RESP_CAPSET: u32 = 0x205;
pub const BROCCOLINI_RESP_ERROR: u32 = 0x206;

// Bitflags for broccolini_resp_resource_create::flags
pub const BROCCOLINI_RESOURCE_FLAG_MAP_INFO: u32 = 1 << 0;
pub const BROCCOLINI_RESOURCE_FLAG_VULKAN_INFO: u32 = 1 << 1;
pub const BROCCOLINI_RESOURCE_FLAG_3D_INFO: u32 = 1 << 2;

/// Handle type tag for Android HardwareBuffer (matches RUTABAGA_HANDLE_TYPE_PLATFORM_AHB).
pub const BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB: u32 = 0x0300_0000;

/// Maximum number of descriptors allowed in a single AHB transfer.
pub const BROCCOLINI_MAX_AHB_FDS: usize = crate::util::TUBE_MAX_IDENTIFIERS;

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
    pub num_fds: u32,
    pub metadata_size: u32,
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
    pub num_fds: u32,
    pub metadata_size: u32,
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

/// Maps to `RutabagaContext::context_create_fence` and async worker fence completion.
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
pub enum BroccoliniHandle {
    Magma(MagmaGpuHandle),
    Ahb {
        fds: Vec<OwnedDescriptor>,
        metadata: Vec<u8>,
    },
}

trait ResourceHandleCmd: FromBytes + IntoBytes + Immutable {
    fn set_handle_header(&mut self, handle_type: u32, num_fds: u32, metadata_size: u32);
}

impl ResourceHandleCmd for broccolini_resp_resource_create {
    fn set_handle_header(&mut self, handle_type: u32, num_fds: u32, metadata_size: u32) {
        self.handle_type = handle_type;
        self.num_fds = num_fds;
        self.metadata_size = metadata_size;
    }
}

impl ResourceHandleCmd for broccolini_ctx_attach_resource {
    fn set_handle_header(&mut self, handle_type: u32, num_fds: u32, metadata_size: u32) {
        self.handle_type = handle_type;
        self.num_fds = num_fds;
        self.metadata_size = metadata_size;
    }
}

fn write_with_resource_handle<T: ResourceHandleCmd>(
    stream: &mut Stream<BroccoliniProtocol>,
    mut cmd: T,
    handle: BroccoliniHandle,
) -> Result<()> {
    match handle {
        BroccoliniHandle::Magma(magma_handle) => {
            cmd.set_handle_header(magma_handle.handle_type, 1, 0);
            stream.write(Message::CmdWithHandle(cmd, magma_handle))
        }
        BroccoliniHandle::Ahb { fds, metadata } => {
            if fds.is_empty() || fds.len() > BROCCOLINI_MAX_AHB_FDS || metadata.is_empty() {
                return Err(Error::InvalidMagmaHandle);
            }
            let num_fds = u32::try_from(fds.len())?;
            let metadata_size = u32::try_from(metadata.len())?;
            cmd.set_handle_header(BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB, num_fds, metadata_size);
            let handles = fds
                .into_iter()
                .map(|os_handle| MagmaGpuHandle {
                    os_handle,
                    handle_type: 0,
                })
                .collect();
            stream.write(Message::CmdWithDataAndHandles(cmd, &metadata, handles))
        }
    }
}

pub fn write_resp_resource_create(
    stream: &mut Stream<BroccoliniProtocol>,
    resp: broccolini_resp_resource_create,
    handle: BroccoliniHandle,
) -> Result<()> {
    write_with_resource_handle(stream, resp, handle)
}

pub fn write_ctx_attach_resource(
    stream: &mut Stream<BroccoliniProtocol>,
    cmd: broccolini_ctx_attach_resource,
    handle: BroccoliniHandle,
) -> Result<()> {
    write_with_resource_handle(stream, cmd, handle)
}

fn decode_resource_handle(
    handle_type: u32,
    num_fds: u32,
    metadata_size: u32,
    reader: &mut Reader<'_>,
    descriptors: &mut VecDeque<OwnedDescriptor>,
) -> Result<BroccoliniHandle> {
    if handle_type == BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB {
        let num_fds: usize = num_fds.try_into()?;
        let metadata_size: usize = metadata_size.try_into()?;
        if num_fds == 0
            || num_fds > BROCCOLINI_MAX_AHB_FDS
            || metadata_size == 0
            || descriptors.len() < num_fds
            || reader.available_bytes() < metadata_size
        {
            return Err(Error::InvalidMagmaHandle);
        }
        let mut metadata = vec![0u8; metadata_size];
        reader.read_exact(&mut metadata)?;
        let mut fds = Vec::with_capacity(num_fds);
        for _ in 0..num_fds {
            fds.push(descriptors.pop_front().ok_or(Error::InvalidMagmaHandle)?);
        }
        Ok(BroccoliniHandle::Ahb { fds, metadata })
    } else {
        if num_fds != 1 || metadata_size != 0 {
            return Err(Error::InvalidMagmaHandle);
        }
        let os_handle = descriptors.pop_front().ok_or(Error::Unsupported)?;
        Ok(BroccoliniHandle::Magma(MagmaGpuHandle {
            os_handle,
            handle_type,
        }))
    }
}

#[derive(Debug)]
pub enum BroccoliniProtocol {
    // Zygote messages
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
    RespResourceCreate(broccolini_resp_resource_create, BroccoliniHandle),
    CtxAttachResource(broccolini_ctx_attach_resource, BroccoliniHandle),
    CtxDetachResource(u32 /* resource_id */),
    CmdSubmit3d {
        cmd_buf: Vec<u8>,
        fence_ids: Vec<u64>,
        shareable_fences: Vec<MagmaGpuHandle>,
    },
    CreateFence(broccolini_create_fence),
    RespCreateFence(u64 /* fence_id */, MagmaGpuHandle),
    CtxDestroy,
    OkNoData,
    RespError,
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
            BROCCOLINI_CMD_GET_CAPSET_INFO => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::GetCapsetInfo(hdr.payload)
            }
            BROCCOLINI_RESP_CAPSET_INFO => BroccoliniProtocol::RespCapsetInfo(reader.read_obj()?),
            BROCCOLINI_CMD_GET_CAPSET => BroccoliniProtocol::GetCapset(reader.read_obj()?),
            BROCCOLINI_RESP_CAPSET => {
                let len: usize = hdr.payload.try_into()?;
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                if reader.available_bytes() < len {
                    return Err(Error::Unsupported);
                }
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
                let handle = decode_resource_handle(
                    resp.handle_type,
                    resp.num_fds,
                    resp.metadata_size,
                    reader,
                    descriptors,
                )?;
                BroccoliniProtocol::RespResourceCreate(resp, handle)
            }
            BROCCOLINI_CMD_CTX_ATTACH_RESOURCE => {
                let req: broccolini_ctx_attach_resource = reader.read_obj()?;
                let handle = decode_resource_handle(
                    req.handle_type,
                    req.num_fds,
                    req.metadata_size,
                    reader,
                    descriptors,
                )?;
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
            BROCCOLINI_CMD_CREATE_FENCE => BroccoliniProtocol::CreateFence(reader.read_obj()?),
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
            BROCCOLINI_RESP_ERROR => {
                reader.consume(size_of::<broccolini_ctrl_hdr>());
                BroccoliniProtocol::RespError
            }
            _ => {
                return Err(Error::Unsupported);
            }
        };

        Ok(protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::create_pipe;
    use crate::util::AsBorrowedDescriptor;
    use zerocopy::IntoBytes;

    fn make_descriptors(count: usize) -> VecDeque<OwnedDescriptor> {
        let mut fds = VecDeque::with_capacity(count);
        for _ in 0..count {
            let (read_pipe, _write_pipe) = create_pipe().expect("create_pipe failed");
            let desc = read_pipe
                .as_borrowed_descriptor()
                .try_clone()
                .expect("try_clone failed");
            fds.push_back(desc);
        }
        fds
    }

    #[test]
    fn decode_ahb_resp_resource_create_round_trip() {
        let metadata = [0x11u8, 0x22, 0x33, 0x44, 0x55];
        let resp = broccolini_resp_resource_create {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_RESP_RESOURCE_CREATE,
                payload: 0,
            },
            resource_id: 7,
            handle_type: BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB,
            num_fds: 2,
            metadata_size: metadata.len() as u32,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(resp.as_bytes());
        buf.extend_from_slice(&metadata);

        let mut reader = Reader::new(&buf);
        let mut descriptors = make_descriptors(2);
        let decoded = BroccoliniProtocol::decode(&mut reader, &mut descriptors).unwrap();
        assert_eq!(reader.available_bytes(), 0);
        assert!(descriptors.is_empty());

        match decoded {
            BroccoliniProtocol::RespResourceCreate(
                decoded_resp,
                BroccoliniHandle::Ahb {
                    fds,
                    metadata: decoded_meta,
                },
            ) => {
                assert_eq!(decoded_resp.resource_id, 7);
                assert_eq!(fds.len(), 2);
                assert_eq!(decoded_meta, metadata);
            }
            other => panic!("unexpected decoded variant: {other:?}"),
        }
    }

    #[test]
    fn decode_ahb_rejects_invalid_bounds() {
        let make_attach_bytes =
            |handle_type: u32, num_fds: u32, metadata_size: u32, payload: &[u8]| {
                let req = broccolini_ctx_attach_resource {
                    hdr: broccolini_ctrl_hdr {
                        type_: BROCCOLINI_CMD_CTX_ATTACH_RESOURCE,
                        payload: 0,
                    },
                    resource_id: 9,
                    handle_type,
                    num_fds,
                    metadata_size,
                    ..Default::default()
                };
                let mut buf = Vec::new();
                buf.extend_from_slice(req.as_bytes());
                buf.extend_from_slice(payload);
                buf
            };

        // Zero FDs, too many FDs, zero metadata, truncated metadata, short descriptors.
        let cases = [
            (
                BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB,
                0,
                4,
                &[1u8, 2, 3, 4][..],
                1,
            ),
            (
                BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB,
                (BROCCOLINI_MAX_AHB_FDS + 1) as u32,
                4,
                &[1u8, 2, 3, 4][..],
                1,
            ),
            (BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB, 1, 0, &[][..], 1),
            (
                BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB,
                1,
                8,
                &[1u8, 2, 3, 4][..],
                1,
            ),
            (
                BROCCOLINI_HANDLE_TYPE_PLATFORM_AHB,
                2,
                4,
                &[1u8, 2, 3, 4][..],
                1,
            ),
            // Non-AHB must have num_fds == 1 and metadata_size == 0.
            (1, 2, 0, &[][..], 2),
            (1, 1, 4, &[1u8, 2, 3, 4][..], 1),
        ];

        for (handle_type, num_fds, metadata_size, payload, desc_count) in cases {
            let buf = make_attach_bytes(handle_type, num_fds, metadata_size, payload);
            let mut reader = Reader::new(&buf);
            let mut descriptors = make_descriptors(desc_count);
            assert!(
                BroccoliniProtocol::decode(&mut reader, &mut descriptors).is_err(),
                "expected error for handle_type={handle_type} num_fds={num_fds} metadata_size={metadata_size}"
            );
        }
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn write_and_decode_resource_handle_round_trip() {
        use crate::util::Tube;

        let (tx_tube, rx_fd) = Tube::pair().expect("Tube::pair failed");
        let mut tx = Stream::<BroccoliniProtocol>::new(tx_tube);
        let mut rx = Stream::<BroccoliniProtocol>::new(
            Tube::try_from(rx_fd).expect("Tube::try_from failed"),
        );

        // Round-trip AHB via write_resp_resource_create.
        let metadata = vec![0xaa, 0xbb, 0xcc, 0xdd];
        let resp = broccolini_resp_resource_create {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_RESP_RESOURCE_CREATE,
                payload: 0,
            },
            resource_id: 11,
            ..Default::default()
        };
        write_resp_resource_create(
            &mut tx,
            resp,
            BroccoliniHandle::Ahb {
                fds: make_descriptors(2).into(),
                metadata: metadata.clone(),
            },
        )
        .unwrap();
        let msgs = rx.read().unwrap();
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BroccoliniProtocol::RespResourceCreate(
                decoded_resp,
                BroccoliniHandle::Ahb {
                    fds,
                    metadata: decoded_meta,
                },
            ) => {
                assert_eq!(decoded_resp.resource_id, 11);
                assert_eq!(fds.len(), 2);
                assert_eq!(decoded_meta, &metadata);
            }
            other => panic!("unexpected message: {other:?}"),
        }

        // Round-trip AHB via write_ctx_attach_resource.
        let attach = broccolini_ctx_attach_resource {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_CTX_ATTACH_RESOURCE,
                payload: 0,
            },
            resource_id: 12,
            ..Default::default()
        };
        write_ctx_attach_resource(
            &mut tx,
            attach,
            BroccoliniHandle::Ahb {
                fds: make_descriptors(1).into(),
                metadata: metadata.clone(),
            },
        )
        .unwrap();
        let msgs = rx.read().unwrap();
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BroccoliniProtocol::CtxAttachResource(
                decoded_req,
                BroccoliniHandle::Ahb {
                    fds,
                    metadata: decoded_meta,
                },
            ) => {
                assert_eq!(decoded_req.resource_id, 12);
                assert_eq!(fds.len(), 1);
                assert_eq!(decoded_meta, &metadata);
            }
            other => panic!("unexpected message: {other:?}"),
        }

        // Reject invalid AHB bounds on write.
        assert!(write_ctx_attach_resource(
            &mut tx,
            attach,
            BroccoliniHandle::Ahb {
                fds: Vec::new(),
                metadata: metadata.clone(),
            },
        )
        .is_err());
        assert!(write_ctx_attach_resource(
            &mut tx,
            attach,
            BroccoliniHandle::Ahb {
                fds: make_descriptors(1).into(),
                metadata: Vec::new(),
            },
        )
        .is_err());
    }

    #[test]
    fn decode_resp_capset_rejects_oversized_length() {
        let hdr = broccolini_ctrl_hdr {
            type_: BROCCOLINI_RESP_CAPSET,
            payload: u32::MAX,
        };
        let mut reader = Reader::new(hdr.as_bytes());
        let mut descriptors = VecDeque::new();
        assert!(BroccoliniProtocol::decode(&mut reader, &mut descriptors).is_err());
    }
}
