// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::MappedRegion;
use zerocopy::IntoBytes;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaImportHandleInfo;
use crate::encoder::Encoder;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::CreateAddressSpace;
use crate::protocol::CreateBuffer;
use crate::protocol::CreateDevice;
use crate::protocol::CreateQueue;
use crate::protocol::CreateSyncObj;
use crate::protocol::MagmaCommandHeader;
use crate::protocol::MagmaCreateSyncObjInfo;
use crate::protocol::MagmaHeapBudget;
use crate::protocol::MagmaSyncObjType;
use crate::protocol::MagmaVirtCapabilities;
use crate::protocol::VirtioCreateRing;
use crate::protocol::VirtioPing;
use crate::protocol::MAGMA_OPCODE_CREATE_ADDRESS_SPACE;
use crate::protocol::MAGMA_OPCODE_CREATE_BUFFER;
use crate::protocol::MAGMA_OPCODE_CREATE_DEVICE;
use crate::protocol::MAGMA_OPCODE_CREATE_QUEUE;
use crate::protocol::MAGMA_OPCODE_CREATE_SYNC_OBJ;
use crate::protocol::MAGMA_OPCODE_VIRTIO_CREATE_RING;
use crate::protocol::MAGMA_OPCODE_VIRTIO_PING;
use crate::ring::MagmaRingBuffer;
use crate::sys::platform::PlatformDevice;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendDevice;
use crate::traits::BackendQueue;
use crate::traits::BackendSyncObject;
use crate::traits::GenericDevice;
use crate::virtgpu_shared::memory::VirtioGpuAddressSpace;
use crate::virtgpu_shared::memory::VirtioGpuBuffer;
use crate::virtgpu_shared::queue::VirtioGpuQueue;
use crate::virtgpu_shared::sync::SyncObjectTracker;
use crate::virtgpu_shared::sync::VirtioGpuSyncObject;
use crate::virtgpu_shared::validate_ring_buffer_len;
use crate::virtgpu_shared::VirtioGpuTransport;

pub struct VirtioGpu<T: VirtioGpuTransport> {
    pub transport: Arc<T>,
    pub caps: MagmaVirtCapabilities,
    pub object_id: AtomicU32,
    pub ring_counter: AtomicU32,
    pub cpu_ring: MagmaRingBuffer,
    pub _cpu_bo_handle: u32,
    pub _cpu_mapping: Arc<dyn MappedRegion>,
}

impl<T: VirtioGpuTransport> VirtioGpu<T> {
    pub fn new(transport: Arc<T>) -> Result<VirtioGpu<T>> {
        transport.init_context()?;

        let ring_size = validate_ring_buffer_len(transport.caps().ring_buffer_len)?;
        let (cpu_bo_handle, cpu_res_handle) = transport.create_blob(0, ring_size)?;
        let cpu_mapping = transport.map_blob(cpu_bo_handle, ring_size)?;
        let cpu_ring = unsafe { MagmaRingBuffer::from_raw_parts(cpu_mapping.as_ptr(), ring_size)? };

        let device = VirtioGpu {
            caps: *transport.caps(),
            transport,
            object_id: AtomicU32::new(1),
            ring_counter: AtomicU32::new(0),
            cpu_ring,
            _cpu_bo_handle: cpu_bo_handle,
            _cpu_mapping: cpu_mapping,
        };

        device.create_ring(0, cpu_res_handle, 1)?;

        let create_dev = CreateDevice {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_CREATE_DEVICE,
                size: std::mem::size_of::<CreateDevice>() as u32,
            },
            physical_device: device.transport.pdev_idx(),
            device: 1,
        };
        device.submit_encoded_cpu_cmd(create_dev.as_bytes())?;

        Ok(device)
    }

    pub fn create_ring(
        &self,
        channel_id: u32,
        ring_blob_id: u32,
        exec_ring_idx: u32,
    ) -> Result<()> {
        let req = VirtioCreateRing {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_VIRTIO_CREATE_RING,
                size: std::mem::size_of::<VirtioCreateRing>() as u32,
            },
            channel_id,
            ring_blob_id,
        };
        self.submit_encoded_cmd(req.as_bytes(), exec_ring_idx, &[])
    }

    pub fn ping_ring(&self, channel_id: u32, exec_ring_idx: u32) -> Result<()> {
        let req = VirtioPing {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_VIRTIO_PING,
                size: std::mem::size_of::<VirtioPing>() as u32,
            },
            channel_id,
            _padding: 0,
        };
        self.submit_encoded_cmd(req.as_bytes(), exec_ring_idx, &[])
    }

    pub fn submit_encoded_cmd(&self, cmd: &[u8], ring_idx: u32, syncobjs: &[u32]) -> Result<()> {
        self.transport.submit_cmd(cmd, ring_idx, syncobjs)
    }

    pub fn submit_encoded_cpu_cmd(&self, cmd: &[u8]) -> Result<()> {
        self.cpu_ring
            .write_and_ping_locked(cmd, || self.ping_ring(0, 1))
    }
}

impl<T: VirtioGpuTransport> PlatformDevice for VirtioGpu<T> {}
impl<T: VirtioGpuTransport> BackendDevice for VirtioGpu<T> {}

impl<T: VirtioGpuTransport> GenericDevice for VirtioGpu<T> {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget> {
        if let Some(heap) = self.transport.device().memory_heaps.get(heap_idx as usize) {
            Ok(MagmaHeapBudget {
                budget: heap.heap_size,
                usage: 0,
            })
        } else {
            Err(Error::InvalidArgs)
        }
    }

    fn create_address_space(self: Arc<VirtioGpu<T>>) -> Result<Arc<dyn BackendAddressSpace>> {
        let object_id = self.object_id.fetch_add(1, Ordering::Relaxed);
        let req = CreateAddressSpace {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_CREATE_ADDRESS_SPACE,
                size: std::mem::size_of::<CreateAddressSpace>() as u32,
            },
            device: 0,
            address_space: object_id,
        };
        self.submit_encoded_cpu_cmd(req.as_bytes())?;
        Ok(Arc::new(VirtioGpuAddressSpace {
            device: self,
            object_id,
        }))
    }

    fn create_queue(
        self: Arc<VirtioGpu<T>>,
        address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let object_id = self.object_id.fetch_add(1, Ordering::Relaxed);
        let as_id = address_space.as_vm_id().unwrap_or(1);
        let req = CreateQueue {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_CREATE_QUEUE,
                size: std::mem::size_of::<CreateQueue>() as u32,
            },
            device: 0,
            address_space: as_id,
            info: *info,
            queue: object_id,
            _padding: 0,
        };
        self.submit_encoded_cpu_cmd(req.as_bytes())?;

        let ring_size = validate_ring_buffer_len(self.caps.ring_buffer_len)?;
        let (bo_handle, res_handle) = self.transport.create_blob(0, ring_size)?;
        let mapping = self.transport.map_blob(bo_handle, ring_size)?;
        let ring = unsafe { MagmaRingBuffer::from_raw_parts(mapping.as_ptr(), ring_size)? };

        let ring_idx = (self.ring_counter.fetch_add(1, Ordering::Relaxed) % 63) + 1;

        self.create_ring(ring_idx, res_handle, ring_idx)?;

        Ok(Arc::new(VirtioGpuQueue {
            device: self,
            object_id,
            ring_idx,
            ring,
            _bo_handle: bo_handle,
            _mapping: mapping,
        }))
    }

    fn create_buffer(
        self: Arc<VirtioGpu<T>>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let object_id = self.object_id.fetch_add(1, Ordering::Relaxed);
        let map_info = self
            .transport
            .device()
            .memory_types
            .get(create_info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());

        let cmd = CreateBuffer {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_CREATE_BUFFER,
                size: std::mem::size_of::<CreateBuffer>() as u32,
            },
            device: 0,
            _pad0: 0,
            info: *create_info,
            buffer_out: object_id,
            _padding: 0,
        };
        let mut buf = [0u8; std::mem::size_of::<CreateBuffer>()];
        let mut encoder = Encoder::from_slice(&mut buf);
        encoder.encode_create_buffer(&cmd)?;
        let written = encoder.bytes_written();
        self.submit_encoded_cpu_cmd(&buf[..written])?;

        Ok(Arc::new(VirtioGpuBuffer {
            device: self,
            object_id,
            bo_handle: AtomicU32::new(0),
            _res_handle: AtomicU32::new(0),
            blob_lock: Mutex::new(()),
            size: create_info.size as usize,
            map_info,
        }))
    }

    fn import(
        self: Arc<VirtioGpu<T>>,
        _info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        Err(Error::Unimplemented)
    }

    fn create_sync_obj(
        self: Arc<VirtioGpu<T>>,
        info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let object_id = self.object_id.fetch_add(1, Ordering::Relaxed);
        let cmd = CreateSyncObj {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_CREATE_SYNC_OBJ,
                size: std::mem::size_of::<CreateSyncObj>() as u32,
            },
            device: 0,
            _pad0: 0,
            info: *info,
            sync_obj: object_id,
            _padding: 0,
        };
        let mut buf = [0u8; std::mem::size_of::<CreateSyncObj>()];
        let mut encoder = Encoder::from_slice(&mut buf);
        encoder.encode_create_sync_obj(&cmd)?;
        let written = encoder.bytes_written();
        self.submit_encoded_cpu_cmd(&buf[..written])?;

        let sync_type = MagmaSyncObjType::from(info.sync_type);
        let guest_syncobj_handle = self.transport.syncobj_create(info.initial_point > 0)?;

        Ok(Arc::new(VirtioGpuSyncObject {
            device: self,
            object_id,
            guest_syncobj_handle,
            use_flags: info.use_flags,
            sync_type,
            tracker: SyncObjectTracker::new(info.initial_point),
        }))
    }
}
