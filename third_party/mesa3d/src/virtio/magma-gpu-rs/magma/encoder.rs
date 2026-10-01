// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT
//
// Generated via:
//   https://gitlab.freedesktop.org/mesa/mesa/-/tree/main/src/magma/gorgonzola
//
// Submit patches, do not hand-edit.
use crate::protocol::*;
pub struct Encoder<W> {
    writer: W,
    bytes_written: usize,
}

impl<W: std::io::Write> Encoder<W> {
    pub fn new(writer: W) -> Encoder<W> {
        Encoder {
            writer,
            bytes_written: 0,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn writer(&self) -> &W {
        &self.writer
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.bytes_written += bytes.len();
        Ok(())
    }
    pub fn encode_extensible_chain(&mut self, mut curr: u64) -> std::io::Result<()> {
        while curr != 0 {
            let header = unsafe { &*(curr as *const MagmaStructureTypeHeader) };
            let next = header.p_next;
            match header.stype {
                MAGMA_STRUCTURE_TYPE_CREATE_BUFFER_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaCreateBufferInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_DEVICE_CREATE_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaDeviceCreateInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_SUBMIT_SYNC_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaWireSubmitSyncInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_SUBMIT_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaWireSubmitInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_SUBMIT_ADDRESS_SPACE_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaSubmitAddressSpaceInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_SUBMIT_BUFFER_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaWireSubmitBufferInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_CREATE_SYNC_OBJ_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaCreateSyncObjInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_SYNC_PROPERTIES => {
                    let mut ext = unsafe { *(curr as *const MagmaSyncProperties) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_VIRT_CAPABILITIES => {
                    let mut ext = unsafe { *(curr as *const MagmaVirtCapabilities) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_INFO => {
                    let mut ext = unsafe { *(curr as *const MagmaPhysicalDeviceInfo) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_TYPE => {
                    let mut ext = unsafe { *(curr as *const MagmaPhysicalDeviceMemoryType) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_HEAP => {
                    let mut ext = unsafe { *(curr as *const MagmaPhysicalDeviceMemoryHeap) };
                    ext.header.p_next = if next != 0 { 1 } else { 0 };
                    self.write_bytes(zerocopy::IntoBytes::as_bytes(&ext))?;
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unknown stype",
                    ))
                }
            }
            curr = next;
        }
        Ok(())
    }
    pub fn encode_create_buffer_info(
        &mut self,
        msg: &MagmaCreateBufferInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_device_create_info(
        &mut self,
        msg: &MagmaDeviceCreateInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_wire_submit_sync_info(
        &mut self,
        msg: &MagmaWireSubmitSyncInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_wire_submit_info(&mut self, msg: &MagmaWireSubmitInfo) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_submit_address_space_info(
        &mut self,
        msg: &MagmaSubmitAddressSpaceInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_wire_submit_buffer_info(
        &mut self,
        msg: &MagmaWireSubmitBufferInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_create_sync_obj_info(
        &mut self,
        msg: &MagmaCreateSyncObjInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_sync_properties(&mut self, msg: &MagmaSyncProperties) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_virt_capabilities(&mut self, msg: &MagmaVirtCapabilities) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_physical_device_info(
        &mut self,
        msg: &MagmaPhysicalDeviceInfo,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_physical_device_memory_type(
        &mut self,
        msg: &MagmaPhysicalDeviceMemoryType,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_physical_device_memory_heap(
        &mut self,
        msg: &MagmaPhysicalDeviceMemoryHeap,
    ) -> std::io::Result<()> {
        let mut obj = *msg;
        let p_next = obj.header.p_next;
        obj.header.p_next = if p_next != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&obj))?;
        if p_next != 0 {
            self.encode_extensible_chain(p_next)?;
        }
        Ok(())
    }
    pub fn encode_create_device(&mut self, msg: &CreateDevice) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_create_device(&mut self, msg: &CreateDeviceResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_get_memory_budget(&mut self, msg: &GetMemoryBudget) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_get_memory_budget(
        &mut self,
        msg: &GetMemoryBudgetResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_create_buffer(&mut self, msg: &CreateBuffer) -> std::io::Result<()> {
        let mut cmd = *msg;
        let p_next_info = cmd.info.header.p_next;
        cmd.info.header.p_next = if p_next_info != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&cmd))?;
        if p_next_info != 0 {
            self.encode_extensible_chain(p_next_info)?;
        }
        Ok(())
    }
    pub fn encode_resp_create_buffer(&mut self, msg: &CreateBufferResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_create_address_space(&mut self, msg: &CreateAddressSpace) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_create_address_space(
        &mut self,
        msg: &CreateAddressSpaceResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_create_queue(&mut self, msg: &CreateQueue) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_create_queue(&mut self, msg: &CreateQueueResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_device_close(&mut self, msg: &DeviceClose) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_physical_device_close(
        &mut self,
        msg: &PhysicalDeviceClose,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_buffer_close(&mut self, msg: &BufferClose) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_queue_close(&mut self, msg: &QueueClose) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_address_space_close(&mut self, msg: &AddressSpaceClose) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_virtio_create_ring(&mut self, msg: &VirtioCreateRing) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_map_buffer_gpu(&mut self, msg: &MapBufferGpu) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_map_buffer_gpu(&mut self, msg: &MapBufferGpuResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_unmap_buffer_gpu(&mut self, msg: &UnmapBufferGpu) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_unmap_buffer_gpu(
        &mut self,
        msg: &UnmapBufferGpuResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_submit_command(&mut self, msg: &SubmitCommand) -> std::io::Result<()> {
        let mut cmd = *msg;
        let p_next_submit_info = cmd.submit_info.header.p_next;
        cmd.submit_info.header.p_next = if p_next_submit_info != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&cmd))?;
        if p_next_submit_info != 0 {
            self.encode_extensible_chain(p_next_submit_info)?;
        }
        Ok(())
    }
    pub fn encode_resp_submit_command(&mut self, msg: &SubmitCommandResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_create_sync_obj(&mut self, msg: &CreateSyncObj) -> std::io::Result<()> {
        let mut cmd = *msg;
        let p_next_info = cmd.info.header.p_next;
        cmd.info.header.p_next = if p_next_info != 0 { 1 } else { 0 };
        self.write_bytes(zerocopy::IntoBytes::as_bytes(&cmd))?;
        if p_next_info != 0 {
            self.encode_extensible_chain(p_next_info)?;
        }
        Ok(())
    }
    pub fn encode_resp_create_sync_obj(&mut self, msg: &CreateSyncObjResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_sync_obj_close(&mut self, msg: &SyncObjClose) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_sync_obj_signal(&mut self, msg: &SyncObjSignal) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_sync_obj_signal(&mut self, msg: &SyncObjSignalResp) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_sync_obj_timeline_wait(
        &mut self,
        msg: &SyncObjTimelineWait,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_sync_obj_timeline_wait(
        &mut self,
        msg: &SyncObjTimelineWaitResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_sync_obj_timeline_signal(
        &mut self,
        msg: &SyncObjTimelineSignal,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_sync_obj_timeline_signal(
        &mut self,
        msg: &SyncObjTimelineSignalResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_sync_obj_timeline_query(
        &mut self,
        msg: &SyncObjTimelineQuery,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_sync_obj_timeline_query(
        &mut self,
        msg: &SyncObjTimelineQueryResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_queue_check_status(&mut self, msg: &QueueCheckStatus) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_resp_queue_check_status(
        &mut self,
        msg: &QueueCheckStatusResp,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_virtio_ping(&mut self, msg: &VirtioPing) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_virtio_create_fence(&mut self, msg: &VirtioCreateFence) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_virtio_sync_obj_close(
        &mut self,
        msg: &VirtioSyncObjClose,
    ) -> std::io::Result<()> {
        self.write_bytes(zerocopy::IntoBytes::as_bytes(msg))
    }
    pub fn encode_msg(&mut self, msg: &MagmaProtocol) -> std::io::Result<()> {
        match msg {
            MagmaProtocol::CreateDevice(msg) => self.encode_create_device(msg),
            MagmaProtocol::GetMemoryBudget(msg) => self.encode_get_memory_budget(msg),
            MagmaProtocol::CreateBuffer(msg) => self.encode_create_buffer(msg),
            MagmaProtocol::CreateAddressSpace(msg) => self.encode_create_address_space(msg),
            MagmaProtocol::CreateQueue(msg) => self.encode_create_queue(msg),
            MagmaProtocol::DeviceClose(msg) => self.encode_device_close(msg),
            MagmaProtocol::PhysicalDeviceClose(msg) => self.encode_physical_device_close(msg),
            MagmaProtocol::BufferClose(msg) => self.encode_buffer_close(msg),
            MagmaProtocol::QueueClose(msg) => self.encode_queue_close(msg),
            MagmaProtocol::AddressSpaceClose(msg) => self.encode_address_space_close(msg),
            MagmaProtocol::VirtioCreateRing(msg) => self.encode_virtio_create_ring(msg),
            MagmaProtocol::MapBufferGpu(msg) => self.encode_map_buffer_gpu(msg),
            MagmaProtocol::UnmapBufferGpu(msg) => self.encode_unmap_buffer_gpu(msg),
            MagmaProtocol::SubmitCommand(msg) => self.encode_submit_command(msg),
            MagmaProtocol::CreateSyncObj(msg) => self.encode_create_sync_obj(msg),
            MagmaProtocol::SyncObjClose(msg) => self.encode_sync_obj_close(msg),
            MagmaProtocol::SyncObjSignal(msg) => self.encode_sync_obj_signal(msg),
            MagmaProtocol::SyncObjTimelineWait(msg) => self.encode_sync_obj_timeline_wait(msg),
            MagmaProtocol::SyncObjTimelineSignal(msg) => self.encode_sync_obj_timeline_signal(msg),
            MagmaProtocol::SyncObjTimelineQuery(msg) => self.encode_sync_obj_timeline_query(msg),
            MagmaProtocol::QueueCheckStatus(msg) => self.encode_queue_check_status(msg),
            MagmaProtocol::VirtioPing(msg) => self.encode_virtio_ping(msg),
            MagmaProtocol::VirtioCreateFence(msg) => self.encode_virtio_create_fence(msg),
            MagmaProtocol::VirtioSyncObjClose(msg) => self.encode_virtio_sync_obj_close(msg),
            MagmaProtocol::CreateDeviceResp(msg) => self.encode_resp_create_device(msg),
            MagmaProtocol::GetMemoryBudgetResp(msg) => self.encode_resp_get_memory_budget(msg),
            MagmaProtocol::CreateBufferResp(msg) => self.encode_resp_create_buffer(msg),
            MagmaProtocol::CreateAddressSpaceResp(msg) => {
                self.encode_resp_create_address_space(msg)
            }
            MagmaProtocol::CreateQueueResp(msg) => self.encode_resp_create_queue(msg),
            MagmaProtocol::MapBufferGpuResp(msg) => self.encode_resp_map_buffer_gpu(msg),
            MagmaProtocol::UnmapBufferGpuResp(msg) => self.encode_resp_unmap_buffer_gpu(msg),
            MagmaProtocol::SubmitCommandResp(msg) => self.encode_resp_submit_command(msg),
            MagmaProtocol::CreateSyncObjResp(msg) => self.encode_resp_create_sync_obj(msg),
            MagmaProtocol::SyncObjSignalResp(msg) => self.encode_resp_sync_obj_signal(msg),
            MagmaProtocol::SyncObjTimelineWaitResp(msg) => {
                self.encode_resp_sync_obj_timeline_wait(msg)
            }
            MagmaProtocol::SyncObjTimelineSignalResp(msg) => {
                self.encode_resp_sync_obj_timeline_signal(msg)
            }
            MagmaProtocol::SyncObjTimelineQueryResp(msg) => {
                self.encode_resp_sync_obj_timeline_query(msg)
            }
            MagmaProtocol::QueueCheckStatusResp(msg) => self.encode_resp_queue_check_status(msg),
        }
    }

    #[allow(dead_code)]
    pub fn encode_extensible_struct(&mut self, msg: &MagmaExtensibleStruct) -> std::io::Result<()> {
        match msg {
            MagmaExtensibleStruct::CreateBufferInfo(msg) => self.encode_create_buffer_info(msg),
            MagmaExtensibleStruct::DeviceCreateInfo(msg) => self.encode_device_create_info(msg),
            MagmaExtensibleStruct::WireSubmitSyncInfo(msg) => {
                self.encode_wire_submit_sync_info(msg)
            }
            MagmaExtensibleStruct::WireSubmitInfo(msg) => self.encode_wire_submit_info(msg),
            MagmaExtensibleStruct::SubmitAddressSpaceInfo(msg) => {
                self.encode_submit_address_space_info(msg)
            }
            MagmaExtensibleStruct::WireSubmitBufferInfo(msg) => {
                self.encode_wire_submit_buffer_info(msg)
            }
            MagmaExtensibleStruct::CreateSyncObjInfo(msg) => self.encode_create_sync_obj_info(msg),
            MagmaExtensibleStruct::SyncProperties(msg) => self.encode_sync_properties(msg),
            MagmaExtensibleStruct::VirtCapabilities(msg) => self.encode_virt_capabilities(msg),
            MagmaExtensibleStruct::PhysicalDeviceInfo(msg) => self.encode_physical_device_info(msg),
            MagmaExtensibleStruct::PhysicalDeviceMemoryType(msg) => {
                self.encode_physical_device_memory_type(msg)
            }
            MagmaExtensibleStruct::PhysicalDeviceMemoryHeap(msg) => {
                self.encode_physical_device_memory_heap(msg)
            }
        }
    }
}

impl<'a> Encoder<&'a mut [u8]> {
    pub fn from_slice(buf: &'a mut [u8]) -> Encoder<&'a mut [u8]> {
        Encoder::new(buf)
    }
}
