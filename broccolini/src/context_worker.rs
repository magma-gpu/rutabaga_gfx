// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::protocols::broccolini_protocol::*;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::EventSignaler;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::WritePipe;
use magma_gpu::util::Writer;
use rutabaga_gfx::DeviceId as RutabagaDeviceId;
use rutabaga_gfx::Resource3DInfo;
use rutabaga_gfx::ResourceCreateBlob;
use rutabaga_gfx::Rutabaga;
use rutabaga_gfx::RutabagaBuilder;
use rutabaga_gfx::RutabagaFence;
use rutabaga_gfx::RutabagaFenceHandler;
use rutabaga_gfx::RutabagaImportData;
use rutabaga_gfx::RutabagaWsi;
use rutabaga_gfx::VulkanInfo as RutabagaVulkanInfo;
use rutabaga_gfx::RUTABAGA_CAPSET_GFXSTREAM_GLES;
use rutabaga_gfx::RUTABAGA_CAPSET_GFXSTREAM_VULKAN;
use rutabaga_gfx::RUTABAGA_CAPSET_VENUS;
use rutabaga_gfx::RUTABAGA_CAPSET_VIRGL;
use rutabaga_gfx::RUTABAGA_CAPSET_VIRGL2;
use rutabaga_gfx::RUTABAGA_FLAG_FENCE_HOST_SHAREABLE;

use crate::sys::WorkerProcess;
use crate::utils::send_ok_nodata;
use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;

pub(crate) fn build_rutabaga(
    capset_mask: u64,
    renderer_features: &str,
    fence_handler: RutabagaFenceHandler,
) -> BroccoliniResult<Rutabaga> {
    let use_gles = (capset_mask & (1 << RUTABAGA_CAPSET_GFXSTREAM_GLES)) != 0
        || (capset_mask & (1 << RUTABAGA_CAPSET_VIRGL)) != 0
        || (capset_mask & (1 << RUTABAGA_CAPSET_VIRGL2)) != 0;
    let use_vulkan = (capset_mask & (1 << RUTABAGA_CAPSET_GFXSTREAM_VULKAN)) != 0
        || (capset_mask & (1 << RUTABAGA_CAPSET_VENUS)) != 0;
    let features_opt = (!renderer_features.is_empty()).then(|| renderer_features.to_string());

    Ok(RutabagaBuilder::new(capset_mask, fence_handler)
        .set_use_external_blob(true)
        .set_use_egl(use_gles)
        .set_use_gles(use_gles)
        .set_use_vulkan(use_vulkan)
        .set_use_surfaceless(true)
        .set_wsi(RutabagaWsi::Surfaceless)
        .set_renderer_features(features_opt)
        .build()?)
}

fn get_context_name(ctx_create: &broccolini_ctx_create) -> String {
    let nlen = (ctx_create.nlen as usize).min(ctx_create.debug_name.len());
    let raw_name = &ctx_create.debug_name[..nlen];
    let trimmed_len = raw_name.iter().position(|&b| b == 0).unwrap_or(nlen);
    String::from_utf8_lossy(&raw_name[..trimmed_len]).into_owned()
}

pub fn run_context_worker_loop(
    mut stream: Stream<BroccoliniProtocol>,
    ctx_create: broccolini_ctx_create,
    fence_write_pipe: OwnedDescriptor,
    capset_mask: u64,
    renderer_features: &str,
) -> BroccoliniResult<()> {
    let ctx_id = ctx_create.hdr.payload;
    if ctx_id == 0 {
        return Err(BroccoliniError::MissingContextId);
    }

    let fence_pipe = Arc::new(Mutex::new(WritePipe::new(fence_write_pipe)));
    let fence_pipe_cb = fence_pipe.clone();
    let fence_signalers: Arc<Mutex<HashMap<u64, EventSignaler>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let fence_signalers_cb = fence_signalers.clone();

    let fence_handler = RutabagaFenceHandler::new(move |fence: RutabagaFence| {
        if let Ok(pipe) = fence_pipe_cb.lock() {
            let cmd = broccolini_create_fence {
                hdr: broccolini_ctrl_hdr {
                    type_: BROCCOLINI_CMD_CREATE_FENCE,
                    payload: fence.ctx_id,
                },
                flags: fence.flags,
                ring_idx: fence.ring_idx,
                padding: [0; 3],
                fence_id: fence.fence_id,
            };
            let mut buf = [0u8; size_of::<broccolini_create_fence>()];
            let mut writer = Writer::new(&mut buf);
            if writer.write_obj(cmd).is_ok() {
                let _ = pipe.write(&buf);
            }
        }
        if let Ok(mut map) = fence_signalers_cb.lock() {
            if let Some(signaler) = map.remove(&fence.fence_id) {
                let _ = signaler.signal();
            }
        }
    });

    let debug_name = get_context_name(&ctx_create);
    WorkerProcess::set_current_name(&debug_name);

    let ctx_capset_id = ctx_create.context_init & 0x00ff;
    let worker_capset_mask = if ctx_capset_id != 0 && (capset_mask & (1u64 << ctx_capset_id)) != 0 {
        1u64 << ctx_capset_id
    } else {
        capset_mask
    };
    let mut rutabaga =
        build_rutabaga(worker_capset_mask, renderer_features, fence_handler.clone())?;

    rutabaga.create_context(
        ctx_id,
        ctx_create.context_init,
        (!debug_name.is_empty()).then_some(debug_name.as_str()),
    )?;

    let pid = WorkerProcess::current_id();
    eprintln!("broccolini: worker pid={pid} initialized for ctx_id={ctx_id} ('{debug_name}')");

    loop {
        let protocols = stream.read()?;

        let mut should_exit = false;
        for msg in protocols {
            match msg {
                BroccoliniProtocol::Terminate => {
                    should_exit = true;
                    break;
                }
                BroccoliniProtocol::CtxDestroy => {
                    let _ = rutabaga.destroy_context(ctx_id);
                    let _ = send_ok_nodata(&mut stream);
                    should_exit = true;
                    break;
                }
                BroccoliniProtocol::ResourceCreateBlob(blob_req, handle_opt) => {
                    let res_id = blob_req.resource_id;
                    let create_blob = ResourceCreateBlob {
                        blob_mem: blob_req.blob_mem,
                        blob_flags: blob_req.blob_flags,
                        blob_id: blob_req.blob_id,
                        size: blob_req.size,
                    };
                    rutabaga.resource_create_blob(
                        ctx_id,
                        res_id,
                        create_blob,
                        None,
                        handle_opt.map(Into::into),
                    )?;

                    let mut flags = 0u32;
                    let map_info = if let Ok(info) = rutabaga.map_info(res_id) {
                        flags |= BROCCOLINI_RESOURCE_FLAG_MAP_INFO;
                        info
                    } else {
                        0
                    };
                    let vk_info = if let Ok(info) = rutabaga.vulkan_info(res_id) {
                        flags |= BROCCOLINI_RESOURCE_FLAG_VULKAN_INFO;
                        info
                    } else {
                        RutabagaVulkanInfo::default()
                    };
                    let info_3d = if let Ok(info) = rutabaga.resource3d_info(res_id) {
                        flags |= BROCCOLINI_RESOURCE_FLAG_3D_INFO;
                        info
                    } else {
                        Resource3DInfo {
                            width: 0,
                            height: 0,
                            drm_fourcc: 0,
                            strides: [0; 4],
                            offsets: [0; 4],
                            modifier: 0,
                        }
                    };
                    let exported = rutabaga.export_blob(res_id)?;
                    let handle = MagmaGpuHandle::try_from(exported)?;

                    let resp = broccolini_resp_resource_create {
                        hdr: broccolini_ctrl_hdr {
                            type_: BROCCOLINI_RESP_RESOURCE_CREATE,
                            payload: 0,
                        },
                        resource_id: res_id,
                        handle_type: handle.handle_type,
                        flags,
                        map_info,
                        vulkan_info: VulkanInfo {
                            memory_idx: vk_info.memory_idx,
                            device_id: DeviceId {
                                device_uuid: vk_info.device_id.device_uuid,
                                driver_uuid: vk_info.device_id.driver_uuid,
                            },
                        },
                        width: info_3d.width,
                        height: info_3d.height,
                        drm_fourcc: info_3d.drm_fourcc,
                        strides: info_3d.strides,
                        offsets: info_3d.offsets,
                        modifier: info_3d.modifier,
                    };
                    stream.write(Message::CmdWithHandle(resp, handle))?;
                }
                BroccoliniProtocol::CtxAttachResource(attach_req, handle) => {
                    let import_data = RutabagaImportData {
                        flags: attach_req.flags,
                        info_3d: Resource3DInfo {
                            width: attach_req.width,
                            height: attach_req.height,
                            drm_fourcc: attach_req.drm_fourcc,
                            strides: attach_req.strides,
                            offsets: attach_req.offsets,
                            modifier: attach_req.modifier,
                        },
                        info_vulkan: RutabagaVulkanInfo {
                            memory_idx: attach_req.vulkan_info.memory_idx,
                            device_id: RutabagaDeviceId {
                                device_uuid: attach_req.vulkan_info.device_id.device_uuid,
                                driver_uuid: attach_req.vulkan_info.device_id.driver_uuid,
                            },
                        },
                        size: attach_req.size,
                    };
                    rutabaga.resource_import(attach_req.resource_id, handle.into(), import_data)?;
                    let _ = rutabaga.context_attach_resource(ctx_id, attach_req.resource_id);
                    send_ok_nodata(&mut stream)?;
                }
                BroccoliniProtocol::CtxDetachResource(res_id) => {
                    let _ = rutabaga.context_detach_resource(ctx_id, res_id);
                    let _ = rutabaga.unref_resource(res_id);
                }
                BroccoliniProtocol::CmdSubmit3d {
                    mut cmd_buf,
                    fence_ids,
                    shareable_fences: _,
                } => {
                    rutabaga.submit_command(ctx_id, &mut cmd_buf, &fence_ids)?;
                }
                BroccoliniProtocol::CreateFence(req, handle_opt) => {
                    let has_handle = handle_opt.is_some();
                    if let Some(handle) = handle_opt {
                        let signaler = EventSignaler::try_from(handle)?;
                        fence_signalers
                            .lock()
                            .map_err(|_| BroccoliniError::FailedToLockFenceSignalers)?
                            .insert(req.fence_id, signaler);
                    }
                    let fence = RutabagaFence {
                        flags: if has_handle {
                            req.flags & !RUTABAGA_FLAG_FENCE_HOST_SHAREABLE
                        } else {
                            req.flags
                        },
                        fence_id: req.fence_id,
                        ctx_id,
                        ring_idx: req.ring_idx,
                    };
                    if let Err(e) = rutabaga.create_fence(fence) {
                        fence_handler.call(fence);
                        return Err(e.into());
                    }
                    if !has_handle && (req.flags & RUTABAGA_FLAG_FENCE_HOST_SHAREABLE != 0) {
                        let handle = rutabaga.export_fence(req.fence_id)?;
                        let _ = rutabaga.destroy_fences(&[req.fence_id]);
                        let resp = broccolini_resp_create_fence {
                            hdr: broccolini_ctrl_hdr {
                                type_: BROCCOLINI_RESP_CREATE_FENCE,
                                payload: handle.handle_type,
                            },
                            fence_id: req.fence_id,
                        };
                        stream.write(Message::CmdWithHandle(resp, handle))?;
                    }
                }
                _ => {
                    send_ok_nodata(&mut stream)?;
                }
            }
        }

        if should_exit {
            break;
        }
    }

    let _ = rutabaga.destroy_context(ctx_id);
    Ok(())
}
