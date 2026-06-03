// Copyright 2026 Magma-GPU project
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap as Map;
use std::convert::TryInto;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use log::error;

use mesa3d_util::create_pipe;
use mesa3d_util::AsBorrowedDescriptor;
use mesa3d_util::Event;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::OwnedDescriptor;
use mesa3d_util::Tube;
use mesa3d_util::TubeType;
use mesa3d_util::WaitContext;
use mesa3d_util::WritePipe;

use zerocopy::FromBytes;
use zerocopy::IntoBytes;

use crate::context_common::ContextResource;
use crate::context_common::ContextResources;
use crate::cross_domain::common::CROSS_DOMAIN_CONTEXT_CHANNEL_ID;
use crate::cross_domain::common::CrossDomainItem;
use crate::cross_domain::common::CrossDomainItemState;
use crate::cross_domain::common::CrossDomainJob;
use crate::cross_domain::common::CrossDomainState;
use crate::cross_domain::common::RingWrite;
use crate::cross_domain::common::SentinelManager;
use crate::cross_domain::common::add_item;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CHANNEL_RING;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CHANNEL_TYPE_INTERNAL_SOCKET;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_ASSIGN_SOCKET_UUID;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_CREATE_ATOMIC_MEMORY_SENTINEL;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_DESTROY_ATOMIC_MEMORY_SENTINEL;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_GET_IMAGE_REQUIREMENTS;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_IMPORT_VIRTIOFS_HANDLE;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_INIT;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_POLL;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_READ_CREATE_EVENT;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_SEND;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_SIGNAL_ATOMIC_MEMORY_SENTINEL;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_WRITE;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_READ_PIPE;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_VIRTGPU_BLOB;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_VIRTIO_FS_BLOB;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_MAX_IDENTIFIERS;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_QUERY_RING;
use crate::cross_domain::cross_domain_protocol::CrossDomainAssignSocketUuid;
use crate::cross_domain::cross_domain_protocol::CrossDomainCreateAtomicMemorySentinel;
use crate::cross_domain::cross_domain_protocol::CrossDomainCreateEvent;
use crate::cross_domain::cross_domain_protocol::CrossDomainDestroyAtomicMemorySentinel;
use crate::cross_domain::cross_domain_protocol::CrossDomainGetImageRequirements;
use crate::cross_domain::cross_domain_protocol::CrossDomainHeader;
use crate::cross_domain::cross_domain_protocol::CrossDomainImageRequirements;
use crate::cross_domain::cross_domain_protocol::CrossDomainImportVirtioFsHandle;
use crate::cross_domain::cross_domain_protocol::CrossDomainInit;
use crate::cross_domain::cross_domain_protocol::CrossDomainInitLegacy;
use crate::cross_domain::cross_domain_protocol::CrossDomainInitV1;
use crate::cross_domain::cross_domain_protocol::CrossDomainReadWrite;
use crate::cross_domain::cross_domain_protocol::CrossDomainSendReceive;
use crate::cross_domain::cross_domain_protocol::CrossDomainSignalAtomicMemorySentinel;
use crate::cross_domain::worker::CrossDomainWorker;
use crate::handle::RutabagaHandle;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_core::RutabagaResource;
use crate::rutabaga_core::VirtioFsLookup;
use crate::rutabaga_utils::Resource3DInfo;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaPath;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::RUTABAGA_BLOB_MEM_GUEST;
use crate::rutabaga_utils::RUTABAGA_MAP_ACCESS_RW;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_CACHED;
use crate::DrmFormat;
use crate::ImageAllocationInfo;
use crate::RutabagaGralloc;
use crate::RutabagaGrallocFlags;

pub struct CrossDomainContext {
    pub paths: Option<Vec<RutabagaPath>>,
    pub gralloc: Arc<Mutex<RutabagaGralloc>>,
    pub state: Option<Arc<CrossDomainState>>,
    pub context_resources: ContextResources,
    pub item_state: CrossDomainItemState,
    pub sentinel_manager: SentinelManager,
    pub fence_handler: RutabagaFenceHandler,
    pub virtiofs_lookup: Option<Arc<dyn VirtioFsLookup>>,
    pub internal_sockets: Arc<Mutex<Map<u128, Tube>>>,
    pub worker_thread: Option<thread::JoinHandle<RutabagaResult<()>>>,
    pub resample_evt: Option<Event>,
    pub kill_evt: Option<Event>,
}

impl CrossDomainContext {
    fn get_connection(&mut self, cmd_init: &CrossDomainInit) -> RutabagaResult<Tube> {
        let paths = self
            .paths
            .take()
            .ok_or(RutabagaError::InvalidCrossDomainChannel)?;
        let path = &paths
            .iter()
            .find(|path| path.path_type == cmd_init.channel_type)
            .ok_or(RutabagaError::InvalidCrossDomainChannel)?
            .path;

        let tube = Tube::new(path.clone(), TubeType::Stream)?;
        Ok(tube)
    }

    fn get_internal_socket_connection(&mut self, uuid: u128) -> RutabagaResult<Tube> {
        let mut sockets = self.internal_sockets.lock().unwrap();
        let socket = sockets
            .remove(&uuid)
            .ok_or(RutabagaError::InvalidCrossDomainInternalSocketUuid)?;

        Ok(socket)
    }

    fn initialize(&mut self, cmd_init: &CrossDomainInit) -> RutabagaResult<()> {
        if !self
            .context_resources
            .lock()
            .unwrap()
            .contains_key(&cmd_init.query_ring_id)
        {
            return Err(RutabagaError::InvalidResourceId);
        }

        let query_ring_id = cmd_init.query_ring_id;
        let channel_ring_id = cmd_init.channel_ring_id;
        let context_resources = self.context_resources.clone();
        let sentinel_manager = self.sentinel_manager.clone();

        // Zero means no requested channel.
        if cmd_init.channel_type != 0 {
            if !self
                .context_resources
                .lock()
                .unwrap()
                .contains_key(&cmd_init.channel_ring_id)
            {
                return Err(RutabagaError::InvalidResourceId);
            }

            let connection = if cmd_init.channel_type == CROSS_DOMAIN_CHANNEL_TYPE_INTERNAL_SOCKET {
                self.get_internal_socket_connection(u128::from_le_bytes(
                    cmd_init.internal_socket_uuid,
                ))?
            } else {
                self.get_connection(cmd_init)?
            };

            let kill_evt = Event::new()?;
            let thread_kill_evt = kill_evt.try_clone()?;

            let resample_evt = Event::new()?;
            let thread_resample_evt = resample_evt.try_clone()?;

            let mut wait_ctx = WaitContext::new()?;
            wait_ctx.add(
                CROSS_DOMAIN_CONTEXT_CHANNEL_ID,
                connection.as_borrowed_descriptor(),
            )?;

            let state = Arc::new(CrossDomainState::new(
                query_ring_id,
                channel_ring_id,
                context_resources,
                sentinel_manager,
                Some(connection),
            ));

            let thread_state = state.clone();
            let thread_items = self.item_state.clone();
            let thread_fence_handler = self.fence_handler.clone();

            let worker_result = thread::Builder::new()
                .name("cross domain".to_string())
                .spawn(move || -> RutabagaResult<()> {
                    CrossDomainWorker::new(
                        wait_ctx,
                        thread_state,
                        thread_items,
                        thread_fence_handler,
                    )
                    .run(thread_kill_evt, thread_resample_evt)
                });

            self.worker_thread = Some(worker_result.unwrap());
            self.state = Some(state);
            self.resample_evt = Some(resample_evt);
            self.kill_evt = Some(kill_evt);
        } else {
            self.state = Some(Arc::new(CrossDomainState::new(
                query_ring_id,
                channel_ring_id,
                context_resources,
                sentinel_manager,
                None,
            )));
        }

        Ok(())
    }

    fn get_image_requirements(
        &mut self,
        cmd_get_reqs: &CrossDomainGetImageRequirements,
    ) -> RutabagaResult<()> {
        let info = ImageAllocationInfo {
            width: cmd_get_reqs.width,
            height: cmd_get_reqs.height,
            drm_format: DrmFormat::from(cmd_get_reqs.drm_format),
            flags: RutabagaGrallocFlags::new(cmd_get_reqs.flags),
        };

        let reqs = self
            .gralloc
            .lock()
            .unwrap()
            .get_image_memory_requirements(info)?;

        let mut response = CrossDomainImageRequirements {
            strides: reqs.strides,
            offsets: reqs.offsets,
            modifier: reqs.modifier,
            size: reqs.size,
            blob_id: 0,
            map_info: reqs.map_info,
            memory_idx: -1,
            physical_device_idx: -1,
        };

        if let Some(ref vk_info) = reqs.vulkan_info {
            response.memory_idx = vk_info.memory_idx as i32;
            // We return -1 for now since physical_device_idx is deprecated. If this backend is
            // put back into action, it should be using device_id from the request instead.
            response.physical_device_idx = -1;
        }

        if let Some(state) = &self.state {
            response.blob_id = add_item(&self.item_state, CrossDomainItem::ImageRequirements(reqs));
            state.write_to_ring(RingWrite::Write(response, None), state.query_ring_id)?;
            Ok(())
        } else {
            Err(RutabagaError::InvalidCrossDomainState)
        }
    }

    fn send(
        &mut self,
        cmd_send: &mut CrossDomainSendReceive,
        opaque_data: &[u8],
    ) -> RutabagaResult<()> {
        let mut descriptors: Vec<OwnedDescriptor> = vec![];
        let mut write_pipe_opt: Option<WritePipe> = None;
        let mut read_pipe_id_opt: Option<u32> = None;

        let num_identifiers = cmd_send.num_identifiers as usize;

        if num_identifiers > CROSS_DOMAIN_MAX_IDENTIFIERS {
            return Err(MesaError::WithContext("max cross domain identifiers exceeded").into());
        }

        let iter = cmd_send
            .identifiers
            .iter_mut()
            .zip(cmd_send.identifier_types.iter_mut())
            .zip(cmd_send.identifier_sizes.iter_mut())
            .map(|((i, it), is)| (i, it, is))
            .take(num_identifiers);

        for (identifier, identifier_type, _identifier_size) in iter {
            if *identifier_type == CROSS_DOMAIN_ID_TYPE_VIRTGPU_BLOB {
                let context_resources = self.context_resources.lock().unwrap();

                let context_resource = context_resources
                    .get(identifier)
                    .ok_or(RutabagaError::InvalidResourceId)?;

                if let Some(mesa_handle) = context_resource
                    .handle
                    .as_ref()
                    .and_then(|h| h.as_mesa_handle())
                {
                    descriptors.push(
                        mesa_handle
                            .os_handle
                            .try_clone()
                            .map_err(MesaError::IoError)?,
                    );
                } else {
                    return Err(MesaError::InvalidMesaHandle.into());
                }
            } else if *identifier_type == CROSS_DOMAIN_ID_TYPE_READ_PIPE {
                // In practice, just 1 pipe pair per send is observed.  If we encounter
                // more, this can be changed later.
                if write_pipe_opt.is_some() {
                    return Err(MesaError::WithContext("expected just one pipe pair").into());
                }

                let (read_pipe, write_pipe) = create_pipe()?;

                descriptors.push(
                    write_pipe
                        .as_borrowed_descriptor()
                        .try_clone()
                        .map_err(MesaError::IoError)?,
                );
                let read_pipe_id: u32 = add_item(
                    &self.item_state,
                    CrossDomainItem::WaylandReadPipe(read_pipe),
                );

                // For Wayland read pipes, the guest guesses which identifier the host will use to
                // avoid waiting for the host to generate one.  Validate guess here.  This works
                // because of the way Sommelier copy + paste works.  If the Sommelier sequence of
                // events changes, it's always possible to wait for the host
                // response.
                if read_pipe_id != *identifier {
                    return Err(RutabagaError::InvalidCrossDomainItemId);
                }

                // The write pipe needs to be dropped after the send_msg(..) call is complete, so
                // the read pipe can receive subsequent hang-up events.
                write_pipe_opt = Some(write_pipe);
                read_pipe_id_opt = Some(read_pipe_id);
            } else if *identifier_type == CROSS_DOMAIN_ID_TYPE_VIRTIO_FS_BLOB {
                if let Some(CrossDomainItem::RegularFile(file)) =
                    self.item_state.lock().unwrap().table.remove(identifier)
                {
                    descriptors.push(file);
                } else {
                    return Err(RutabagaError::InvalidCrossDomainItemId);
                }
            } else {
                // Don't know how to handle anything else yet.
                return Err(RutabagaError::InvalidCrossDomainItemType);
            }
        }

        if let (Some(state), Some(ref mut resample_evt)) = (&self.state, &mut self.resample_evt) {
            state.send_msg(opaque_data, &descriptors)?;

            if let Some(read_pipe_id) = read_pipe_id_opt {
                state.add_job(CrossDomainJob::AddReadPipe(read_pipe_id));
                resample_evt.signal()?;
            }
        } else {
            return Err(RutabagaError::InvalidCrossDomainState);
        }

        Ok(())
    }

    fn atomic_memory_sentinel_signal(
        &mut self,
        cmd_atomic_memory_sentinel_signal: &CrossDomainSignalAtomicMemorySentinel,
    ) -> RutabagaResult<()> {
        let manager = self.sentinel_manager.lock().unwrap();
        manager.signal_watcher(cmd_atomic_memory_sentinel_signal.id)
    }

    fn atomic_memory_sentinel_destroy(
        &mut self,
        cmd_atomic_memory_sentinel_destroy: &CrossDomainDestroyAtomicMemorySentinel,
    ) -> RutabagaResult<()> {
        let mut manager = self.sentinel_manager.lock().unwrap();
        manager.destroy_watcher(cmd_atomic_memory_sentinel_destroy.id)
    }

    fn atomic_memory_sentinel_new(
        &mut self,
        cmd_atomic_memory_sentinel_new: &CrossDomainCreateAtomicMemorySentinel,
    ) -> RutabagaResult<()> {
        let id = cmd_atomic_memory_sentinel_new.id;
        let fs_id = cmd_atomic_memory_sentinel_new.fs_id;
        let handle = cmd_atomic_memory_sentinel_new.handle;

        let mut manager = self.sentinel_manager.lock().unwrap();
        let evt = manager.create_watcher(id, fs_id, handle)?;

        let state = self
            .state
            .as_ref()
            .ok_or(RutabagaError::InvalidCrossDomainState)?;
        state.add_job(CrossDomainJob::AddAtomicMemorySentinel(id, evt));

        Ok(())
    }

    fn read_event_new(&mut self, cmd_event_new: &CrossDomainCreateEvent) -> RutabagaResult<()> {
        let items = self.item_state.lock().unwrap();

        if let Some(item) = items.table.get(&cmd_event_new.id) {
            if let CrossDomainItem::Event(_) = item {
                self.state
                    .as_ref()
                    .unwrap()
                    .add_job(CrossDomainJob::AddReadEvent(cmd_event_new.id));
                self.resample_evt.as_mut().unwrap().signal()?;
                Ok(())
            } else {
                Err(RutabagaError::InvalidCrossDomainItemType)
            }
        } else {
            Err(RutabagaError::InvalidCrossDomainItemId)
        }
    }

    fn import_virtiofs_handle(
        &mut self,
        cmd_imp_handle: &CrossDomainImportVirtioFsHandle,
    ) -> RutabagaResult<()> {
        let mut items = self.item_state.lock().unwrap();

        if items.table.contains_key(&cmd_imp_handle.id) {
            return Err(RutabagaError::InvalidCrossDomainItemId);
        }

        let Some(ref lookup) = self.virtiofs_lookup else {
            return Err(RutabagaError::InvalidCrossDomainState);
        };

        let file = lookup.get_exported_descriptor(cmd_imp_handle.fs_id, cmd_imp_handle.handle)?;

        items
            .table
            .insert(cmd_imp_handle.id, CrossDomainItem::RegularFile(file));

        Ok(())
    }

    fn socket_assign_uuid(
        &mut self,
        cmd_exp_socket: &CrossDomainAssignSocketUuid,
    ) -> RutabagaResult<()> {
        let mut items = self.item_state.lock().unwrap();

        let item = items
            .table
            .remove(&cmd_exp_socket.id)
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

        if let CrossDomainItem::Socket(fd) = item {
            let mut sockets = self.internal_sockets.lock().unwrap();
            sockets.insert(
                u128::from_le_bytes(cmd_exp_socket.socket_uuid),
                Tube::try_from(fd)?,
            );
            Ok(())
        } else {
            Err(RutabagaError::InvalidCrossDomainItemType)
        }
    }

    fn write(&self, cmd_write: &CrossDomainReadWrite, opaque_data: &[u8]) -> RutabagaResult<()> {
        let mut items = self.item_state.lock().unwrap();

        // Most of the time, hang-up and writing will be paired.  In lieu of this, remove the
        // item rather than getting a reference.  In case of an error, there's not much to do
        // besides reporting it.
        let item = items
            .table
            .remove(&cmd_write.identifier)
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

        let len: usize = cmd_write
            .opaque_data_size
            .try_into()
            .map_err(MesaError::TryFromIntError)?;
        match item {
            CrossDomainItem::WaylandWritePipe(write_pipe) => {
                if len != 0 {
                    write_pipe.write(opaque_data)?;
                }

                if cmd_write.hang_up == 0 {
                    items.table.insert(
                        cmd_write.identifier,
                        CrossDomainItem::WaylandWritePipe(write_pipe),
                    );
                }

                Ok(())
            }
            CrossDomainItem::Event(mut event) => {
                let Ok(bytes) = <[u8; 8]>::try_from(opaque_data) else {
                    return Err(RutabagaError::InvalidCrossDomainWriteLength);
                };

                event.add(u64::from_le_bytes(bytes))?;

                if cmd_write.hang_up == 0 {
                    items
                        .table
                        .insert(cmd_write.identifier, CrossDomainItem::Event(event));
                }

                Ok(())
            }
            _ => Err(RutabagaError::InvalidCrossDomainItemType),
        }
    }

    fn process_cmd_send(&mut self, commands: &mut [u8]) -> RutabagaResult<()> {
        let opaque_data_offset = size_of::<CrossDomainSendReceive>();
        let (mut cmd_send, _) = CrossDomainSendReceive::read_from_prefix(commands.as_bytes())
            .map_err(|_| RutabagaError::InvalidCommandBuffer)?;

        let opaque_data = commands
            .get_mut(opaque_data_offset..opaque_data_offset + cmd_send.opaque_data_size as usize)
            .ok_or(RutabagaError::InvalidCommandSize(
                cmd_send.opaque_data_size as usize,
            ))?;

        self.send(&mut cmd_send, opaque_data)?;
        Ok(())
    }
}

impl RutabagaContext for CrossDomainContext {
    fn context_create_blob(
        &mut self,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        let item_id = resource_create_blob.blob_id as u32;

        let mut items = self.item_state.lock().unwrap();
        let item = items
            .table
            .get_mut(&item_id)
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

        // Items that are kept in the table after usage.
        if let CrossDomainItem::ImageRequirements(reqs) = item {
            if reqs.size != resource_create_blob.size {
                return Err(MesaError::WithContext("blob size mismatch").into());
            }

            // Strictly speaking, it's against the virtio-gpu spec to allocate memory in the context
            // create blob function, which says "the actual allocation is done via
            // VIRTIO_GPU_CMD_SUBMIT_3D."  However, atomic resource creation is easiest for the
            // cross-domain use case, so whatever.
            let hnd = match handle_opt {
                Some(handle) => handle,
                None => self.gralloc.lock().unwrap().allocate_memory(*reqs)?.into(),
            };

            let info_3d = Resource3DInfo {
                width: reqs.info.width,
                height: reqs.info.height,
                drm_fourcc: reqs.info.drm_format.into(),
                strides: reqs.strides,
                offsets: reqs.offsets,
                modifier: reqs.modifier,
            };

            // Keep ImageRequirements items and return immediately, since they can be used for subsequent allocations.
            return Ok(RutabagaResource {
                resource_id,
                handle: Some(Arc::new(hnd)),
                blob: true,
                blob_mem: resource_create_blob.blob_mem,
                blob_flags: resource_create_blob.blob_flags,
                map_info: Some(reqs.map_info | RUTABAGA_MAP_ACCESS_RW),
                info_2d: None,
                info_3d: Some(info_3d),
                vulkan_info: reqs.vulkan_info,
                backing_iovecs: None,
                component_mask: 1 << (RutabagaComponentType::CrossDomain as u8),
                size: resource_create_blob.size,
                mapping: None,
            });
        }

        let item = items
            .table
            .remove(&item_id)
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

        // Items that are removed from the table after one usage.
        match item {
            CrossDomainItem::Blob(hnd) => {
                let map_access = hnd
                    .os_handle
                    .determine_map_access_mode()
                    .map_err(|e| RutabagaError::MesaError(e.into()))?;
                let map_info = Some(RUTABAGA_MAP_CACHE_CACHED | map_access);

                Ok(RutabagaResource {
                    resource_id,
                    handle: Some(Arc::new(hnd.into())),
                    blob: true,
                    blob_mem: resource_create_blob.blob_mem,
                    blob_flags: resource_create_blob.blob_flags,
                    map_info,
                    info_2d: None,
                    info_3d: None,
                    vulkan_info: None,
                    backing_iovecs: None,
                    component_mask: 1 << (RutabagaComponentType::CrossDomain as u8),
                    size: resource_create_blob.size,
                    mapping: None,
                })
            }
            _ => Err(RutabagaError::InvalidCrossDomainItemType),
        }
    }

    fn submit_cmd(
        &mut self,
        mut commands: &mut [u8],
        _fence_ids: &[u64],
        _shareable_fences: Vec<MesaHandle>,
    ) -> RutabagaResult<()> {
        while !commands.is_empty() {
            let (hdr, _) = CrossDomainHeader::read_from_prefix(commands)
                .map_err(|_| RutabagaError::InvalidCommandBuffer)?;

            match hdr.cmd {
                CROSS_DOMAIN_CMD_INIT
                    if hdr.cmd_size as usize == size_of::<CrossDomainInitLegacy>() =>
                {
                    let (cmd_init, _) = CrossDomainInitLegacy::read_from_prefix(commands)
                        .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.initialize(&cmd_init.upgrade())?;
                }
                CROSS_DOMAIN_CMD_INIT
                    if hdr.cmd_size as usize == size_of::<CrossDomainInitV1>() =>
                {
                    let (cmd_init, _) = CrossDomainInitV1::read_from_prefix(commands)
                        .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.initialize(&cmd_init.upgrade())?;
                }
                CROSS_DOMAIN_CMD_INIT if hdr.cmd_size as usize == size_of::<CrossDomainInit>() => {
                    let (cmd_init, _) = CrossDomainInit::read_from_prefix(commands)
                        .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.initialize(&cmd_init)?;
                }
                CROSS_DOMAIN_CMD_GET_IMAGE_REQUIREMENTS => {
                    let (cmd_get_reqs, _) =
                        CrossDomainGetImageRequirements::read_from_prefix(commands)
                            .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;

                    self.get_image_requirements(&cmd_get_reqs)?;
                }
                CROSS_DOMAIN_CMD_SEND => {
                    self.process_cmd_send(commands)?;
                }
                CROSS_DOMAIN_CMD_POLL => {
                    // Actual polling is done in the subsequent when creating a fence.
                }
                CROSS_DOMAIN_CMD_WRITE => {
                    let opaque_data_offset = size_of::<CrossDomainReadWrite>();
                    let (cmd_write, _) = CrossDomainReadWrite::read_from_prefix(commands)
                        .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;

                    let opaque_data = commands
                        .get_mut(
                            opaque_data_offset
                                ..opaque_data_offset + cmd_write.opaque_data_size as usize,
                        )
                        .ok_or::<RutabagaError>(RutabagaError::InvalidCommandSize(
                            cmd_write.opaque_data_size as usize,
                        ))?;

                    self.write(&cmd_write, opaque_data)?;
                }
                CROSS_DOMAIN_CMD_CREATE_ATOMIC_MEMORY_SENTINEL => {
                    let (cmd_atomic_memory_sentinel_new, _) =
                        CrossDomainCreateAtomicMemorySentinel::read_from_prefix(commands)
                            .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.atomic_memory_sentinel_new(&cmd_atomic_memory_sentinel_new)?;
                }
                CROSS_DOMAIN_CMD_SIGNAL_ATOMIC_MEMORY_SENTINEL => {
                    let (cmd_atomic_memory_sentinel_signal, _) =
                        CrossDomainSignalAtomicMemorySentinel::read_from_prefix(commands)
                            .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.atomic_memory_sentinel_signal(&cmd_atomic_memory_sentinel_signal)?;
                }
                CROSS_DOMAIN_CMD_DESTROY_ATOMIC_MEMORY_SENTINEL => {
                    let (cmd_atomic_memory_sentinel_destroy, _) =
                        CrossDomainDestroyAtomicMemorySentinel::read_from_prefix(commands)
                            .map_err(|_e| RutabagaError::InvalidCommandBuffer)?;
                    self.atomic_memory_sentinel_destroy(&cmd_atomic_memory_sentinel_destroy)?;
                }
                CROSS_DOMAIN_CMD_READ_CREATE_EVENT => {
                    let (cmd_new_evt, _) = CrossDomainCreateEvent::read_from_prefix(commands)
                        .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                    self.read_event_new(&cmd_new_evt)?;
                }
                CROSS_DOMAIN_CMD_ASSIGN_SOCKET_UUID => {
                    let (cmd_exp_socket, _) =
                        CrossDomainAssignSocketUuid::read_from_prefix(commands.as_bytes())
                            .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                    self.socket_assign_uuid(&cmd_exp_socket)?;
                }
                CROSS_DOMAIN_CMD_IMPORT_VIRTIOFS_HANDLE => {
                    let (cmd_imp_handle, _) =
                        CrossDomainImportVirtioFsHandle::read_from_prefix(commands.as_bytes())
                            .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                    self.import_virtiofs_handle(&cmd_imp_handle)?;
                }
                _ => return Err(MesaError::WithContext("invalid cross domain command").into()),
            }

            commands = commands
                .get_mut(hdr.cmd_size as usize..)
                .ok_or(RutabagaError::InvalidCommandSize(hdr.cmd_size as usize))?;
        }

        Ok(())
    }

    fn attach(&mut self, resource: &mut RutabagaResource) {
        if resource.blob_mem == RUTABAGA_BLOB_MEM_GUEST {
            self.context_resources.lock().unwrap().insert(
                resource.resource_id,
                ContextResource {
                    handle: None,
                    backing_iovecs: resource.backing_iovecs.take(),
                },
            );
        } else if let Some(ref handle) = resource.handle {
            self.context_resources.lock().unwrap().insert(
                resource.resource_id,
                ContextResource {
                    handle: Some(handle.clone()),
                    backing_iovecs: None,
                },
            );
        }
    }

    fn detach(&mut self, resource: &RutabagaResource) {
        self.context_resources
            .lock()
            .unwrap()
            .remove(&resource.resource_id);
    }

    fn context_create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<Option<MesaHandle>> {
        match fence.ring_idx as u32 {
            CROSS_DOMAIN_QUERY_RING => self.fence_handler.call(fence),
            CROSS_DOMAIN_CHANNEL_RING => {
                if let Some(state) = &self.state {
                    state.add_job(CrossDomainJob::HandleFence(fence));
                }
            }
            _ => return Err(MesaError::WithContext("unexpected ring type").into()),
        }

        Ok(None)
    }

    fn component_type(&self) -> RutabagaComponentType {
        RutabagaComponentType::CrossDomain
    }
}

impl Drop for CrossDomainContext {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            state.add_job(CrossDomainJob::Finish);
        }

        if let Some(mut kill_evt) = self.kill_evt.take() {
            // Log the error, but still try to join the worker thread
            match kill_evt.signal() {
                Ok(_) => (),
                Err(e) => {
                    error!("failed to write cross domain kill event: {e}");
                }
            }

            if let Some(worker_thread) = self.worker_thread.take() {
                let _ = worker_thread.join();
            }
        }
    }
}
