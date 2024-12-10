// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;

use mesa3d_util::log_status;
use mesa3d_util::MappedRegion;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaResult;

use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;

use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::GenericBuffer;
use crate::traits::GenericDevice;
use crate::traits::PhysicalDevice;

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMappedMemoryRange;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MagmaPciInfo;
use crate::magma_defines::MAGMA_HEAP_CPU_VISIBLE_BIT;
use crate::magma_defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;

use crate::flexible_array_impl;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::xe_bindings::*;
use crate::sys::linux::flexible_array::FlexibleArray;
use crate::sys::linux::flexible_array::FlexibleArrayWrapper;
use crate::sys::linux::PlatformDevice;

// This information is also useful to the system side of a driver.  Should be separated
// into it's own crate or module.
const GEN12_IDS: [u16; 50] = [
    0x4c8a, 0x4c8b, 0x4c8c, 0x4c90, 0x4c9a, 0x4680, 0x4681, 0x4682, 0x4683, 0x4688, 0x4689, 0x4690,
    0x4691, 0x4692, 0x4693, 0x4698, 0x4699, 0x4626, 0x4628, 0x462a, 0x46a0, 0x46a1, 0x46a2, 0x46a3,
    0x46a6, 0x46a8, 0x46aa, 0x46b0, 0x46b1, 0x46b2, 0x46b3, 0x46c0, 0x46c1, 0x46c2, 0x46c3, 0x9A40,
    0x9A49, 0x9A59, 0x9A60, 0x9A68, 0x9A70, 0x9A78, 0x9AC0, 0x9AC9, 0x9AD9, 0x9AF8, 0x4905, 0x4906,
    0x4907, 0x4908,
];

const ADLP_IDS: [u16; 23] = [
    0x46A0, 0x46A1, 0x46A2, 0x46A3, 0x46A6, 0x46A8, 0x46AA, 0x462A, 0x4626, 0x4628, 0x46B0, 0x46B1,
    0x46B2, 0x46B3, 0x46C0, 0x46C1, 0x46C2, 0x46C3, 0x46D0, 0x46D1, 0x46D2, 0x46D3, 0x46D4,
];

const RPLP_IDS: [u16; 10] = [
    0xA720, 0xA721, 0xA7A0, 0xA7A1, 0xA7A8, 0xA7A9, 0xA7AA, 0xA7AB, 0xA7AC, 0xA7AD,
];

const MTL_IDS: [u16; 5] = [0x7D40, 0x7D60, 0x7D45, 0x7D55, 0x7DD5];

const LNL_IDS: [u16; 3] = [0x6420, 0x64A0, 0x64B0];

const PTL_IDS: [u16; 8] = [
    0xB080, 0xB081, 0xB082, 0xB083, 0xB08F, 0xB090, 0xB0A0, 0xB0B0,
];

ioctl_readwrite!(
    drm_ioctl_xe_device_query,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_DEVICE_QUERY,
    drm_xe_device_query
);

ioctl_readwrite!(
    drm_ioctl_xe_gem_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_GEM_CREATE,
    drm_xe_gem_create
);

ioctl_readwrite!(
    drm_ioctl_xe_gem_mmap_offset,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_GEM_MMAP_OFFSET,
    drm_xe_gem_mmap_offset
);

ioctl_readwrite!(
    drm_ioctl_xe_vm_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_VM_CREATE,
    drm_xe_vm_create
);

ioctl_write_ptr!(
    drm_ioctl_xe_vm_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_VM_DESTROY,
    drm_xe_vm_destroy
);

flexible_array_impl!(drm_xe_query_config, __u64, num_params, info);
flexible_array_impl!(
    drm_xe_query_mem_regions,
    drm_xe_mem_region,
    num_mem_regions,
    mem_regions
);

pub struct Xe {
    physical_device: Arc<dyn PhysicalDevice>,
    _gtt_size: u64,
    _mem_alignment: u64,
    mem_props: MagmaMemoryProperties,
    sysmem_instance: u16,
    vram_instance: u16,
}

struct XeBuffer {
    physical_device: Arc<dyn PhysicalDevice>,
    gem_handle: u32,
    size: usize,
}

struct XeContext {
    physical_device: Arc<dyn PhysicalDevice>,
    vm_id: u32,
}

fn xe_device_query<T, S>(
    physical_device: &Arc<dyn PhysicalDevice>,
    query_id: u32,
) -> MesaResult<FlexibleArrayWrapper<T, S>>
where
    T: FlexibleArray<S> + Default,
{
    let mut device_query: drm_xe_device_query = drm_xe_device_query {
        query: query_id,
        ..Default::default()
    };

    // SAFETY:
    // Valid arguments are supplied for the following arguments:
    //   - Underlying descriptor
    //   - drm_xe_device_query
    unsafe {
        drm_ioctl_xe_device_query(physical_device.as_fd().unwrap(), &mut device_query)?;
    };

    let total_size = device_query.size;
    let mut wrapper = FlexibleArrayWrapper::<T, S>::from_total_size(total_size as usize);

    // SAFETY:
    // Valid arguments are supplied for the following arguments:
    //   - Underlying descriptor
    //   - drm_xe_device_query
    //   - drm_xe_device_query.data: we trust the FlexibleArrayWrapper to hold enough space
    unsafe {
        device_query.data = wrapper.as_mut_ptr() as __u64;
        drm_ioctl_xe_device_query(physical_device.as_fd().unwrap(), &mut device_query)?;
    };

    Ok(wrapper)
}

/// Determines and sets the graphics version of the Intel device based on its ID.
fn determine_graphics_version(pci_device_id: u16) -> MesaResult<u32> {
    let mut graphics_version = 0;
    if ADLP_IDS.contains(&pci_device_id) {
        graphics_version = 12;
    }

    if RPLP_IDS.contains(&pci_device_id) {
        graphics_version = 12;
    }

    if MTL_IDS.contains(&pci_device_id) {
        graphics_version = 12;
    }

    if LNL_IDS.contains(&pci_device_id) {
        graphics_version = 20;
    }

    if PTL_IDS.contains(&pci_device_id) {
        graphics_version = 20;
    }

    if GEN12_IDS.contains(&pci_device_id) {
        graphics_version = 12;
    }

    if graphics_version != 0 {
        Ok(graphics_version)
    } else {
        Err(MesaError::WithContext("missing intel pci-id"))
    }
}

#[derive(Default)]
struct XeMemoryInfo {
    vram_size: u64,
    vram_used: u64,
    sysmem_size: u64,
    sysmem_used: u64,
    vram_cpu_visible_size: u64,
    vram_cpu_visible_used: u64,
    sysmem_instance: u16,
    vram_instance: u16,
}

fn xe_query_memory_regions(physical_device: &Arc<dyn PhysicalDevice>) -> MesaResult<XeMemoryInfo> {
    let mut memory_info: XeMemoryInfo = Default::default();
    let query_mem_regions = xe_device_query::<drm_xe_query_mem_regions, drm_xe_mem_region>(
        physical_device,
        DRM_XE_DEVICE_QUERY_MEM_REGIONS,
    )?;

    let mem_regions = query_mem_regions.entries_slice();
    for region in mem_regions {
        match region.mem_class as u32 {
            DRM_XE_MEM_REGION_CLASS_SYSMEM => {
                if memory_info.sysmem_size != 0 {
                    return Err(MesaError::WithContext("sysmem_size should not be set"));
                }

                // this should really use sysconf(_SC_PHYS_PAGES) * sysconf(_SC_PAGE_SIZE) for the
                // host-visible heap.  rustix has get_page_size(), but not get_num_pages..
                memory_info.sysmem_size = region.total_size;
                memory_info.sysmem_used = region.used;
                memory_info.sysmem_instance = region.instance;
            }
            DRM_XE_MEM_REGION_CLASS_VRAM => {
                if memory_info.vram_size != 0 || memory_info.vram_cpu_visible_size != 0 {
                    return Err(MesaError::WithContext("one vram value should be zero"));
                }

                memory_info.vram_cpu_visible_size = region.cpu_visible_size;
                memory_info.vram_size = region.total_size - region.cpu_visible_size;
                memory_info.vram_cpu_visible_used = region.cpu_visible_used;
                memory_info.vram_used = region.used - region.cpu_visible_used;
                memory_info.vram_instance = region.instance;
            }
            _ => return Err(MesaError::Unsupported),
        }
    }

    Ok(memory_info)
}

impl Xe {
    pub fn new(
        physical_device: Arc<dyn PhysicalDevice>,
        pci_info: &MagmaPciInfo,
    ) -> MesaResult<Xe> {
        let _graphics_version = determine_graphics_version(pci_info.device_id)?;
        let mut mem_props: MagmaMemoryProperties = Default::default();

        let query_config = xe_device_query::<drm_xe_query_config, __u64>(
            &physical_device,
            DRM_XE_DEVICE_QUERY_CONFIG,
        )?;
        let config = query_config.entries_slice();
        let _config_len = config.len();

        let gtt_size = 1u64 << config[DRM_XE_QUERY_CONFIG_VA_BITS as usize];
        let mem_alignment = config[DRM_XE_QUERY_CONFIG_MIN_ALIGNMENT as usize];

        let memory_info = xe_query_memory_regions(&physical_device)?;
        if memory_info.sysmem_size != 0 {
            // Non-LLC case ignored.
            mem_props.add_heap(memory_info.sysmem_size, MAGMA_HEAP_CPU_VISIBLE_BIT);
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            );

            mem_props.increment_heap_count();
        }

        if memory_info.vram_cpu_visible_size != 0 {
            mem_props.add_heap(
                memory_info.vram_cpu_visible_size,
                MAGMA_HEAP_CPU_VISIBLE_BIT | MAGMA_HEAP_DEVICE_LOCAL_BIT,
            );
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            );

            mem_props.increment_heap_count();
        }

        if memory_info.vram_size != 0 {
            mem_props.add_heap(memory_info.vram_size, MAGMA_HEAP_DEVICE_LOCAL_BIT);
            mem_props.add_memory_type(MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
            mem_props.increment_heap_count();
        }

        Ok(Xe {
            physical_device,
            _gtt_size: gtt_size,
            _mem_alignment: mem_alignment,
            mem_props,
            sysmem_instance: memory_info.sysmem_instance,
            vram_instance: memory_info.vram_instance,
        })
    }
}

impl GenericDevice for Xe {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties> {
        Ok(self.mem_props.clone())
    }

    fn get_memory_budget(&self, heap_idx: u32) -> MesaResult<MagmaHeapBudget> {
        if heap_idx >= self.mem_props.memory_heap_count {
            return Err(MesaError::WithContext("Heap Index out of bounds"));
        }

        let memory_info = xe_query_memory_regions(&self.physical_device)?;
        let heap = &self.mem_props.memory_heaps[heap_idx as usize];

        let (budget, usage) = if heap.is_device_local() && heap.is_cpu_visible() {
            (
                memory_info.vram_cpu_visible_size,
                memory_info.vram_cpu_visible_used,
            )
        } else if heap.is_device_local() {
            (memory_info.vram_size, memory_info.vram_used)
        } else if heap.is_cpu_visible() {
            (memory_info.sysmem_size, memory_info.sysmem_used)
        } else {
            return Err(MesaError::Unsupported);
        };

        Ok(MagmaHeapBudget { budget, usage })
    }

    fn create_context(&self, _device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>> {
        let ctx = XeContext::new(self.physical_device.clone(), 0)?;
        Ok(Arc::new(ctx))
    }

    fn create_buffer(
        &self,
        _device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let buf = XeBuffer::new(
            self.physical_device.clone(),
            create_info,
            &self.mem_props,
            self.sysmem_instance,
            self.vram_instance,
        )?;
        Ok(Arc::new(buf))
    }

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let buf = XeBuffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
        )?;
        Ok(Arc::new(buf))
    }
}

impl PlatformDevice for Xe {}
impl Device for Xe {}

impl XeContext {
    fn new(physical_device: Arc<dyn PhysicalDevice>, _priority: i32) -> MesaResult<XeContext> {
        let mut vm_create = drm_xe_vm_create {
            flags: DRM_XE_VM_CREATE_FLAG_SCRATCH_PAGE,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_xe_vm_create struct
        unsafe {
            drm_ioctl_xe_vm_create(physical_device.as_fd().unwrap(), &mut vm_create)?;
        };

        Ok(XeContext {
            physical_device,
            vm_id: vm_create.vm_id,
        })
    }
}

impl Drop for XeContext {
    fn drop(&mut self) {
        let destroy = drm_xe_vm_destroy {
            vm_id: self.vm_id,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_xe_vm_destroy struct
        let result =
            unsafe { drm_ioctl_xe_vm_destroy(self.physical_device.as_fd().unwrap(), &destroy) };
        log_status!(result);
    }
}

impl Context for XeContext {}

impl XeBuffer {
    fn new(
        physical_device: Arc<dyn PhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_props: &MagmaMemoryProperties,
        sysmem_instance: u16,
        vram_instance: u16,
    ) -> MesaResult<XeBuffer> {
        let mut gem_create: drm_xe_gem_create = Default::default();
        let mut pxp_ext: drm_xe_ext_set_property = Default::default();

        gem_create.size = create_info.size;
        let memory_type = mem_props.get_memory_type(create_info.memory_type_idx);
        let memory_heap = mem_props.get_memory_heap(memory_type.heap_idx);

        if memory_type.is_cached() {
            gem_create.cpu_caching = DRM_XE_GEM_CPU_CACHING_WB as u16;
        } else {
            gem_create.cpu_caching = DRM_XE_GEM_CPU_CACHING_WC as u16;
        }

        if memory_heap.is_cpu_visible() && memory_heap.is_device_local() {
            gem_create.flags |= DRM_XE_GEM_CREATE_FLAG_NEEDS_VISIBLE_VRAM;
            gem_create.placement |= 1 << sysmem_instance;
            gem_create.placement |= 1 << vram_instance;
        } else if memory_heap.is_device_local() {
            gem_create.placement |= 1 << vram_instance;
        } else if memory_heap.is_cpu_visible() {
            gem_create.placement |= 1 << sysmem_instance;
        }

        if memory_type.is_protected() {
            pxp_ext.base.name = DRM_XE_GEM_CREATE_EXTENSION_SET_PROPERTY;
            pxp_ext.property = DRM_XE_GEM_CREATE_SET_PROPERTY_PXP_TYPE;
            pxp_ext.value = DRM_XE_PXP_TYPE_HWDRM as u64;
            gem_create.extensions = &pxp_ext as *const drm_xe_ext_set_property as u64;
        }

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_amdgpu_gem_create_args
        unsafe {
            drm_ioctl_xe_gem_create(physical_device.as_fd().unwrap(), &mut gem_create)?;
        };

        Ok(XeBuffer {
            physical_device,
            gem_handle: gem_create.handle,
            size: create_info.size.try_into()?,
        })
    }

    fn from_existing(
        physical_device: Arc<dyn PhysicalDevice>,
        gem_handle: u32,
        size: usize,
    ) -> MesaResult<XeBuffer> {
        Ok(XeBuffer {
            physical_device,
            gem_handle,
            size,
        })
    }
}

impl GenericBuffer for XeBuffer {
    fn map(&self, _buffer: &Arc<dyn Buffer>) -> MesaResult<Arc<dyn MappedRegion>> {
        let mut xe_offset: drm_xe_gem_mmap_offset = Default::default();

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_xe_gem_mmap_offset
        let offset = unsafe {
            xe_offset.handle = self.gem_handle;
            drm_ioctl_xe_gem_mmap_offset(self.physical_device.as_fd().unwrap(), &mut xe_offset)?;
            xe_offset.offset
        };

        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> MesaResult<MesaHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        Err(MesaError::Unsupported)
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        Err(MesaError::Unsupported)
    }
}

impl Drop for XeBuffer {
    fn drop(&mut self) {
        self.physical_device.close(self.gem_handle)
    }
}

impl Buffer for XeBuffer {}

unsafe impl Send for Xe {}
unsafe impl Sync for Xe {}

unsafe impl Send for XeContext {}
unsafe impl Sync for XeContext {}

unsafe impl Send for XeBuffer {}
unsafe impl Sync for XeBuffer {}
