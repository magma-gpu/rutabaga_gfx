// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT
//
// Generated via:
//   https://gitlab.freedesktop.org/mesa/mesa/-/tree/main/src/magma/gorgonzola
//
// Submit patches, do not hand-edit.
use crate::protocol::*;
use magma_gpu::util::Reader;

pub fn decode(reader: &mut Reader) -> Option<MagmaProtocol> {
    if reader.available_bytes() < std::mem::size_of::<MagmaCommandHeader>() {
        return None;
    }
    let header = reader.peek_obj::<MagmaCommandHeader>().ok()?;
    match header.opcode {
        MAGMA_OPCODE_CREATE_DEVICE => {
            Some(MagmaProtocol::CreateDevice(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_GET_MEMORY_BUDGET => {
            Some(MagmaProtocol::GetMemoryBudget(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_CREATE_BUFFER => {
            Some(MagmaProtocol::CreateBuffer(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_CREATE_ADDRESS_SPACE => Some(MagmaProtocol::CreateAddressSpace(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_CREATE_QUEUE => Some(MagmaProtocol::CreateQueue(reader.read_try_obj().ok()?)),
        MAGMA_OPCODE_DEVICE_CLOSE => Some(MagmaProtocol::DeviceClose(reader.read_try_obj().ok()?)),
        MAGMA_OPCODE_PHYSICAL_DEVICE_CLOSE => Some(MagmaProtocol::PhysicalDeviceClose(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_BUFFER_CLOSE => Some(MagmaProtocol::BufferClose(reader.read_try_obj().ok()?)),
        MAGMA_OPCODE_QUEUE_CLOSE => Some(MagmaProtocol::QueueClose(reader.read_try_obj().ok()?)),
        MAGMA_OPCODE_ADDRESS_SPACE_CLOSE => Some(MagmaProtocol::AddressSpaceClose(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_VIRTIO_CREATE_RING => {
            Some(MagmaProtocol::VirtioCreateRing(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_MAP_BUFFER_GPU => {
            Some(MagmaProtocol::MapBufferGpu(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_UNMAP_BUFFER_GPU => {
            Some(MagmaProtocol::UnmapBufferGpu(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_SUBMIT_COMMAND => {
            Some(MagmaProtocol::SubmitCommand(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_CREATE_SYNC_OBJ => {
            Some(MagmaProtocol::CreateSyncObj(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_SYNC_OBJ_CLOSE => {
            Some(MagmaProtocol::SyncObjClose(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_SYNC_OBJ_SIGNAL => {
            Some(MagmaProtocol::SyncObjSignal(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_SYNC_OBJ_TIMELINE_WAIT => Some(MagmaProtocol::SyncObjTimelineWait(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_SYNC_OBJ_TIMELINE_SIGNAL => Some(MagmaProtocol::SyncObjTimelineSignal(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_SYNC_OBJ_TIMELINE_QUERY => Some(MagmaProtocol::SyncObjTimelineQuery(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_QUEUE_CHECK_STATUS => {
            Some(MagmaProtocol::QueueCheckStatus(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_VIRTIO_PING => Some(MagmaProtocol::VirtioPing(reader.read_try_obj().ok()?)),
        MAGMA_OPCODE_VIRTIO_CREATE_FENCE => Some(MagmaProtocol::VirtioCreateFence(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_VIRTIO_SYNC_OBJ_CLOSE => Some(MagmaProtocol::VirtioSyncObjClose(
            reader.read_try_obj().ok()?,
        )),

        MAGMA_OPCODE_RESP_CREATE_DEVICE => {
            Some(MagmaProtocol::CreateDeviceResp(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_RESP_GET_MEMORY_BUDGET => Some(MagmaProtocol::GetMemoryBudgetResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_CREATE_BUFFER => {
            Some(MagmaProtocol::CreateBufferResp(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_RESP_CREATE_ADDRESS_SPACE => Some(MagmaProtocol::CreateAddressSpaceResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_CREATE_QUEUE => {
            Some(MagmaProtocol::CreateQueueResp(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_RESP_MAP_BUFFER_GPU => {
            Some(MagmaProtocol::MapBufferGpuResp(reader.read_try_obj().ok()?))
        }
        MAGMA_OPCODE_RESP_UNMAP_BUFFER_GPU => Some(MagmaProtocol::UnmapBufferGpuResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_SUBMIT_COMMAND => Some(MagmaProtocol::SubmitCommandResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_CREATE_SYNC_OBJ => Some(MagmaProtocol::CreateSyncObjResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_SYNC_OBJ_SIGNAL => Some(MagmaProtocol::SyncObjSignalResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_WAIT => Some(MagmaProtocol::SyncObjTimelineWaitResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_SIGNAL => Some(
            MagmaProtocol::SyncObjTimelineSignalResp(reader.read_try_obj().ok()?),
        ),
        MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_QUERY => Some(MagmaProtocol::SyncObjTimelineQueryResp(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_OPCODE_RESP_QUEUE_CHECK_STATUS => Some(MagmaProtocol::QueueCheckStatusResp(
            reader.read_try_obj().ok()?,
        )),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn decode_extensible_struct(reader: &mut Reader) -> Option<MagmaExtensibleStruct> {
    if reader.available_bytes() < std::mem::size_of::<MagmaStructureTypeHeader>() {
        return None;
    }
    let header = reader.peek_obj::<MagmaStructureTypeHeader>().ok()?;
    match header.stype {
        MAGMA_STRUCTURE_TYPE_CREATE_BUFFER_INFO => Some(MagmaExtensibleStruct::CreateBufferInfo(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_DEVICE_CREATE_INFO => Some(MagmaExtensibleStruct::DeviceCreateInfo(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_SUBMIT_SYNC_INFO => Some(MagmaExtensibleStruct::WireSubmitSyncInfo(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_SUBMIT_INFO => Some(MagmaExtensibleStruct::WireSubmitInfo(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_SUBMIT_ADDRESS_SPACE_INFO => Some(
            MagmaExtensibleStruct::SubmitAddressSpaceInfo(reader.read_try_obj().ok()?),
        ),
        MAGMA_STRUCTURE_TYPE_SUBMIT_BUFFER_INFO => Some(
            MagmaExtensibleStruct::WireSubmitBufferInfo(reader.read_try_obj().ok()?),
        ),
        MAGMA_STRUCTURE_TYPE_CREATE_SYNC_OBJ_INFO => Some(
            MagmaExtensibleStruct::CreateSyncObjInfo(reader.read_try_obj().ok()?),
        ),
        MAGMA_STRUCTURE_TYPE_SYNC_PROPERTIES => Some(MagmaExtensibleStruct::SyncProperties(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_VIRT_CAPABILITIES => Some(MagmaExtensibleStruct::VirtCapabilities(
            reader.read_try_obj().ok()?,
        )),
        MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_INFO => Some(
            MagmaExtensibleStruct::PhysicalDeviceInfo(reader.read_try_obj().ok()?),
        ),
        MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_TYPE => Some(
            MagmaExtensibleStruct::PhysicalDeviceMemoryType(reader.read_try_obj().ok()?),
        ),
        MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_HEAP => Some(
            MagmaExtensibleStruct::PhysicalDeviceMemoryHeap(reader.read_try_obj().ok()?),
        ),
        _ => None,
    }
}
