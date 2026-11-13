// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

//! remote: IPC client for the multi-process Zygote `broccolini`.
//!
//! Each Virtio-GPU context created by a guest application connects to the Zygote over the
//! initial `server_descriptor` socket, receives a dedicated socket connected to a newly
//! `fork()`ed `broccolini` worker process, and executes all context commands,
//! resource imports, blob creations, and fence waits over `Stream<BroccoliniProtocol>`.

mod component;
mod context;

use std::collections::BTreeMap as Map;
use std::sync::Mutex;

pub use component::RemoteComponent;
use magma_gpu::protocols::broccolini_protocol::*;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::Tube;

use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaResult;

pub struct ZygoteChannel {
    zygote_stream: Mutex<Stream<BroccoliniProtocol>>,
    cached_capsets: Map<u32, (u32, Vec<u8>)>,
}

impl ZygoteChannel {
    pub fn new(
        server_descriptor: OwnedDescriptor,
        remote_capset_mask: u64,
    ) -> RutabagaResult<ZygoteChannel> {
        let tube = Tube::try_from(server_descriptor)?;
        let mut stream = Stream::new(tube);
        let mut cached_capsets = Map::new();

        for capset_id in 0..64u32 {
            if (remote_capset_mask & (1u64 << capset_id)) == 0 {
                continue;
            }
            let (version, size) = Self::query_capset_info(&mut stream, capset_id);
            if size > 0 {
                let data = Self::query_capset(&mut stream, capset_id, version);
                cached_capsets.insert(capset_id, (version, data));
            }
        }

        Ok(ZygoteChannel {
            zygote_stream: Mutex::new(stream),
            cached_capsets,
        })
    }

    fn query_capset_info(stream: &mut Stream<BroccoliniProtocol>, capset_id: u32) -> (u32, u32) {
        let hdr = broccolini_ctrl_hdr {
            type_: BROCCOLINI_CMD_GET_CAPSET_INFO,
            payload: capset_id,
        };
        if stream.write(Message::Cmd(hdr)).is_err() {
            return (0, 0);
        }
        if let Ok(protocols) = stream.read() {
            for protocol in protocols {
                if let BroccoliniProtocol::RespCapsetInfo(info) = protocol {
                    return (info.version, info.size);
                }
            }
        }
        (0, 0)
    }

    fn query_capset(
        stream: &mut Stream<BroccoliniProtocol>,
        capset_id: u32,
        version: u32,
    ) -> Vec<u8> {
        let get_capset = broccolini_get_capset {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_GET_CAPSET,
                payload: 0,
            },
            capset_id,
            capset_version: version,
        };
        if stream.write(Message::Cmd(get_capset)).is_err() {
            return Vec::new();
        }
        if let Ok(protocols) = stream.read() {
            for protocol in protocols {
                if let BroccoliniProtocol::RespCapset(capset) = protocol {
                    return capset;
                }
            }
        }
        Vec::new()
    }

    pub fn get_capset_info(&self, capset_id: u32) -> (u32, u32) {
        if let Some((version, data)) = self.cached_capsets.get(&capset_id) {
            return (*version, data.len() as u32);
        }
        let mut stream = self.zygote_stream.lock().unwrap();
        Self::query_capset_info(&mut stream, capset_id)
    }

    pub fn get_capset(&self, capset_id: u32, version: u32) -> Vec<u8> {
        if let Some((_, data)) = self.cached_capsets.get(&capset_id) {
            return data.clone();
        }
        let mut stream = self.zygote_stream.lock().unwrap();
        Self::query_capset(&mut stream, capset_id, version)
    }

    pub(crate) fn spawn_context(
        &self,
        ctx_id: u32,
        context_init: u32,
        context_name: Option<&str>,
        fence_write_desc: OwnedDescriptor,
    ) -> RutabagaResult<Stream<BroccoliniProtocol>> {
        let mut debug_name = [0u8; 64];
        let mut nlen = 0u32;
        if let Some(name) = context_name {
            let bytes = name.as_bytes();
            let copy_len = std::cmp::min(bytes.len(), 63);
            debug_name[..copy_len].copy_from_slice(&bytes[..copy_len]);
            nlen = copy_len as u32;
        }

        let mut zygote = self.zygote_stream.lock().unwrap();
        let create = broccolini_ctx_create {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_CTX_CREATE,
                payload: ctx_id,
            },
            nlen,
            context_init,
            debug_name,
        };
        zygote.write(Message::CmdWithHandle(
            create,
            MagmaGpuHandle {
                os_handle: fence_write_desc,
                handle_type: 0,
            },
        ))?;

        let mut worker_desc = None;
        let protocols = zygote.read()?;
        for protocol in protocols {
            if let BroccoliniProtocol::RespCtxCreateHandle(_ctx_id, desc) = protocol {
                worker_desc = Some(desc);
            }
        }
        let desc = worker_desc.ok_or(RutabagaError::InvalidContextId)?;
        let tube = Tube::try_from(desc)?;
        Ok(Stream::new(tube))
    }
}
