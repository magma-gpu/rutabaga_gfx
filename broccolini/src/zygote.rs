// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use magma_gpu::protocols::broccolini_protocol::*;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::Tube;
use rutabaga_gfx::RutabagaFenceHandler;

use crate::context_worker::build_rutabaga;
use crate::context_worker::run_context_worker_loop;
use crate::sys::WorkerProcess;
use crate::utils::send_ok_nodata;
use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;

pub struct CachedCapset {
    pub capset_id: u32,
    pub version: u32,
    pub data: Vec<u8>,
}

/// Forks a short-lived probe child to query the configured Rutabaga capsets without
/// initializing GPU drivers in the Zygote process.
pub fn probe_capsets_in_child(
    capset_mask: u64,
    renderer_features: &str,
) -> BroccoliniResult<Vec<CachedCapset>> {
    let worker = WorkerProcess::new(move |mut stream| {
        let noop_handler = RutabagaFenceHandler::new(|_| {});
        let rutabaga = build_rutabaga(capset_mask, renderer_features, noop_handler)?;
        let num_capsets = rutabaga.get_num_capsets();
        for i in 0..num_capsets {
            if let Ok((capset_id, version, size)) = rutabaga.get_capset_info(i) {
                let data = if size > 0 {
                    rutabaga.get_capset(capset_id, version).unwrap_or_default()
                } else {
                    Vec::new()
                };
                let _ = stream.write(Message::Cmd(broccolini_resp_capset_info {
                    hdr: broccolini_ctrl_hdr {
                        type_: BROCCOLINI_RESP_CAPSET_INFO,
                        payload: 0,
                    },
                    capset_id,
                    version,
                    size: data.len() as u32,
                    padding: 0,
                }));
                let _ = stream.write(Message::CmdWithData(
                    broccolini_ctrl_hdr {
                        type_: BROCCOLINI_RESP_CAPSET,
                        payload: data.len() as u32,
                    },
                    &data,
                ));
            }
        }
        std::mem::forget(rutabaga);
        Ok(())
    })?;

    let mut stream = Stream::new(Tube::try_from(worker.into_client_descriptor())?);
    let mut capsets = Vec::new();
    let mut pending_info: Option<broccolini_resp_capset_info> = None;
    let mut done = false;
    while !done {
        for msg in stream.read()? {
            match msg {
                BroccoliniProtocol::RespCapsetInfo(info) => {
                    pending_info = Some(info);
                }
                BroccoliniProtocol::RespCapset(data) => {
                    if let Some(info) = pending_info.take() {
                        capsets.push(CachedCapset {
                            capset_id: info.capset_id,
                            version: info.version,
                            data,
                        });
                    }
                }
                BroccoliniProtocol::Terminate => {
                    done = true;
                    break;
                }
                _ => {}
            }
        }
    }

    if capsets.is_empty() {
        return Err(BroccoliniError::EmptyCapsetsFromProbeChild);
    }

    Ok(capsets)
}

pub fn spawn_context_worker(
    req: broccolini_ctx_create,
    fence_write_pipe: OwnedDescriptor,
    capset_mask: u64,
    renderer_features: &str,
) -> BroccoliniResult<OwnedDescriptor> {
    let worker = WorkerProcess::new(move |stream| {
        run_context_worker_loop(
            stream,
            req,
            fence_write_pipe,
            capset_mask,
            renderer_features,
        )
    })?;
    Ok(worker.into_client_descriptor())
}

pub fn run_render_server_zygote_loop(
    mut broccolini_control_stream: Stream<BroccoliniProtocol>,
    cached_capsets: &[CachedCapset],
    capset_mask: u64,
    renderer_features: &str,
) -> BroccoliniResult<()> {
    loop {
        let messages = broccolini_control_stream.read()?;

        let mut disconnected = false;
        for msg in messages {
            match msg {
                BroccoliniProtocol::Terminate => {
                    disconnected = true;
                    break;
                }
                BroccoliniProtocol::GetNumCapsets => {
                    broccolini_control_stream.write(Message::Cmd(broccolini_ctrl_hdr {
                        type_: BROCCOLINI_RESP_NUM_CAPSETS,
                        payload: cached_capsets.len() as u32,
                    }))?;
                }
                BroccoliniProtocol::GetCapsetInfo(id_or_idx) => {
                    let entry = cached_capsets
                        .iter()
                        .find(|c| c.capset_id == id_or_idx)
                        .or_else(|| cached_capsets.get(id_or_idx as usize));
                    let (capset_id, version, size) = match entry {
                        Some(c) => (c.capset_id, c.version, c.data.len() as u32),
                        None => (0, 0, 0),
                    };
                    broccolini_control_stream.write(Message::Cmd(broccolini_resp_capset_info {
                        hdr: broccolini_ctrl_hdr {
                            type_: BROCCOLINI_RESP_CAPSET_INFO,
                            payload: 0,
                        },
                        capset_id,
                        version,
                        size,
                        padding: 0,
                    }))?;
                }
                BroccoliniProtocol::GetCapset(req) => {
                    let data = cached_capsets
                        .iter()
                        .find(|c| c.capset_id == req.capset_id)
                        .map(|c| c.data.as_slice())
                        .unwrap_or(&[]);
                    broccolini_control_stream.write(Message::CmdWithData(
                        broccolini_ctrl_hdr {
                            type_: BROCCOLINI_RESP_CAPSET,
                            payload: data.len() as u32,
                        },
                        data,
                    ))?;
                }
                BroccoliniProtocol::CtxCreate(req, fence_write_pipe) => {
                    let ctx_id = req.hdr.payload;
                    let client_desc = spawn_context_worker(
                        req,
                        fence_write_pipe,
                        capset_mask,
                        renderer_features,
                    )?;

                    broccolini_control_stream.write(Message::CmdWithHandle(
                        broccolini_ctrl_hdr {
                            type_: BROCCOLINI_RESP_CTX_CREATE_HANDLE,
                            payload: ctx_id,
                        },
                        MagmaGpuHandle {
                            os_handle: client_desc,
                            handle_type: 0,
                        },
                    ))?;
                }
                _ => {
                    send_ok_nodata(&mut broccolini_control_stream)?;
                }
            }
        }

        if disconnected {
            break;
        }
    }
    Ok(())
}
