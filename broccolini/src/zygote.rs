// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use magma_gpu::protocols::broccolini_protocol::*;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;

use crate::sys::WorkerProcess;
use crate::utils::send_ok_nodata;
use crate::utils::BroccoliniResult;

pub fn spawn_context_worker(
    _req: broccolini_ctx_create,
    fence_write_pipe: OwnedDescriptor,
    _capset_mask: u64,
    _renderer_features: &str,
) -> BroccoliniResult<OwnedDescriptor> {
    let worker = WorkerProcess::new(move |stream| {
        drop(fence_write_pipe);
        drop(stream);
        Ok(())
    })?;
    Ok(worker.into_client_descriptor())
}

pub fn run_render_server_zygote_loop(
    mut broccolini_control_stream: Stream<BroccoliniProtocol>,
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
