// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::os::raw::c_void;
use std::slice::from_raw_parts;
use std::sync::Arc;

use libc::wcslen;
use log::error;

use mesa3d_util::IntoRawDescriptor;
use mesa3d_util::MappedRegion;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaMapping;
use mesa3d_util::MesaResult;

use crate::check_ntstatus;
use crate::log_ntstatus;
use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMappedMemoryRange;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MagmaPciBusInfo;
use crate::magma_defines::MagmaPciInfo;
use crate::magma_defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
use crate::magma_defines::MAGMA_SYNC_RANGES;
use crate::magma_defines::MAGMA_SYNC_WHOLE_RANGE;
use crate::magma_defines::MAGMA_VENDOR_ID_AMD;

use crate::sys::windows::Amd;
use crate::sys::windows::VendorPrivateData;

use crate::traits::AsVirtGpu;
use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::GenericBuffer;
use crate::traits::GenericDevice;
use crate::traits::GenericPhysicalDevice;
use crate::traits::PhysicalDevice;

use windows_sys::Wdk::Graphics::Direct3D::*;
use windows_sys::Win32::Foundation::LUID;

type D3dkmtHandle = u32;

pub struct WddmAdapter {
    handle: D3dkmtHandle,
    _luid: LUID,
    segment_group_size: D3DKMT_SEGMENTGROUPSIZEINFO,
    _hw_sch_enabled: bool,
    _hw_sch_supported: bool,
    adapter_name: String,
    chip_type: String,
}

pub struct WddmDevice {
    handle: D3dkmtHandle,
    adapter: Arc<dyn PhysicalDevice>,
    vendor_private_data: Box<dyn VendorPrivateData>,
    mem_props: MagmaMemoryProperties,
}

pub struct WddmBuffer {
    handle: D3dkmtHandle,
    device: Arc<dyn Device>,
    size: u64,
}

pub struct WddmContext {
    handle: D3dkmtHandle,
    _device: Arc<dyn Device>,
}

struct WddmMapping {
    _buffer: Arc<dyn Buffer>,
    pdata: *mut c_void,
    size: usize,
}

pub trait WindowsDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        0
    }

    fn vendor_private_data(&self) -> Option<&dyn VendorPrivateData> {
        None
    }
}

pub trait WindowsPhysicalDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        0
    }

    fn segment_group_size(&self) -> D3DKMT_SEGMENTGROUPSIZEINFO {
        Default::default()
    }
}

impl WddmAdapter {
    pub fn new(handle: D3dkmtHandle, luid: LUID) -> WddmAdapter {
        WddmAdapter {
            handle,
            _luid: luid,
            segment_group_size: Default::default(),
            _hw_sch_enabled: Default::default(),
            _hw_sch_supported: Default::default(),
            adapter_name: Default::default(),
            chip_type: Default::default(),
        }
    }

    pub fn initialize(&mut self) -> MesaResult<(MagmaPciInfo, MagmaPciBusInfo)> {
        let mut pci_info: MagmaPciInfo = Default::default();
        let mut pci_bus_info: MagmaPciBusInfo = Default::default();

        let mut query_device_ids: D3DKMT_QUERY_DEVICE_IDS = Default::default();
        let mut adapter_address: D3DKMT_ADAPTERADDRESS = Default::default();

        let mut adapter_info = D3DKMT_QUERYADAPTERINFO {
            hAdapter: self.handle,
            Type: KMTQAITYPE_PHYSICALADAPTERDEVICEIDS,
            pPrivateDriverData: &mut query_device_ids as *mut D3DKMT_QUERY_DEVICE_IDS
                as *mut c_void,
            PrivateDriverDataSize: std::mem::size_of::<D3DKMT_QUERY_DEVICE_IDS>() as u32,
        };

        // SAFETY:
        //  - `adapter_info` is stack-allocated and properly typed.
        //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
        //      KMTQAITYPE_PHYSICALADAPTERDEVICEIDS operation
        check_ntstatus!(unsafe {
            D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
        })?;

        adapter_info.Type = KMTQAITYPE_ADAPTERADDRESS;
        adapter_info.pPrivateDriverData =
            &mut adapter_address as *mut D3DKMT_ADAPTERADDRESS as *mut c_void;
        adapter_info.PrivateDriverDataSize = std::mem::size_of::<D3DKMT_ADAPTERADDRESS>() as u32;

        // SAFETY:
        //  - `adapter_info` is stack-allocated and properly typed.
        //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
        //      KMTQAITYPE_ADAPTERADDRESS operation
        check_ntstatus!(unsafe {
            D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
        })?;

        let mut wddm_caps: D3DKMT_WDDM_2_7_CAPS = Default::default();
        adapter_info.Type = KMTQAITYPE_WDDM_2_7_CAPS;
        adapter_info.pPrivateDriverData =
            &mut wddm_caps as *mut D3DKMT_WDDM_2_7_CAPS as *mut c_void;
        adapter_info.PrivateDriverDataSize = std::mem::size_of::<D3DKMT_WDDM_2_7_CAPS>() as u32;

        // SAFETY:
        //  - `adapter_info` is stack-allocated and properly typed.
        //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
        //      KMTQAITYPE_WDDM_2_7_CAPS operation
        check_ntstatus!(unsafe {
            D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
        })?;

        adapter_info.Type = KMTQAITYPE_GETSEGMENTGROUPSIZE;
        adapter_info.pPrivateDriverData =
            &mut self.segment_group_size as *mut D3DKMT_SEGMENTGROUPSIZEINFO as *mut c_void;
        adapter_info.PrivateDriverDataSize =
            std::mem::size_of::<D3DKMT_SEGMENTGROUPSIZEINFO>() as u32;

        // SAFETY:
        //  - `adapter_info` is stack-allocated and properly typed.
        //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
        //      KMTQAITYPE_GETSEGMENTGROUPSIZE operation
        check_ntstatus!(unsafe {
            D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
        })?;

        let mut registry_info: D3DKMT_ADAPTERREGISTRYINFO = Default::default();
        adapter_info.Type = KMTQAITYPE_ADAPTERREGISTRYINFO_RENDER;
        adapter_info.pPrivateDriverData =
            &mut registry_info as *mut D3DKMT_ADAPTERREGISTRYINFO as *mut c_void;
        adapter_info.PrivateDriverDataSize =
            std::mem::size_of::<D3DKMT_ADAPTERREGISTRYINFO>() as u32;

        // SAFETY:
        //  - `adapter_info` is stack-allocated and properly typed.
        //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
        //      KMTQAITYPE_ADAPTERREGISTERYINFO operation
        check_ntstatus!(unsafe {
            D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
        })?;

        // SAFETY:
        //  - `registry_info` has been successfully retrieved and contains well-formed UTF-16 data.
        //  -  WCHAR/wchar_t are 16-bits on Windows.
        let adapter_name_len = unsafe { wcslen(&registry_info.AdapterString[0] as *const u16) };
        let chip_type_len = unsafe { wcslen(&registry_info.ChipType[0] as *const u16) };
        let adapter_name_slice: &[u16] = unsafe {
            from_raw_parts(
                &registry_info.AdapterString[0] as *const _,
                adapter_name_len,
            )
        };
        let chip_type_slice: &[u16] =
            unsafe { from_raw_parts(&registry_info.ChipType[0] as *const _, chip_type_len) };

        self.adapter_name = String::from_utf16(adapter_name_slice)
            .map_err(|_| MesaError::WithContext("invalid utf-16 data"))?;
        self.chip_type = String::from_utf16(chip_type_slice)
            .map_err(|_| MesaError::WithContext("invalid utf-16 data"))?;

        let device_ids = query_device_ids.DeviceIds;
        pci_info.revision_id = device_ids.RevisionID.try_into()?;
        pci_info.vendor_id = device_ids.VendorID.try_into()?;
        pci_info.device_id = device_ids.DeviceID.try_into()?;
        pci_info.subvendor_id = device_ids.SubVendorID.try_into()?;
        pci_info.subdevice_id = device_ids.SubSystemID.try_into()?;

        pci_bus_info.domain = 0;
        pci_bus_info.bus = adapter_address.BusNumber.try_into()?;
        pci_bus_info.device = adapter_address.DeviceNumber.try_into()?;
        pci_bus_info.function = adapter_address.FunctionNumber.try_into()?;

        Ok((pci_info, pci_bus_info))
    }
}

impl GenericPhysicalDevice for WddmAdapter {
    fn create_device(
        &self,
        physical_device: &Arc<dyn PhysicalDevice>,
        pci_info: &MagmaPciInfo,
    ) -> MesaResult<Arc<dyn Device>> {
        let vendor_private_data = match pci_info.vendor_id {
            MAGMA_VENDOR_ID_AMD => Box::new(Amd(())),
            _ => todo!(),
        };

        let device = WddmDevice::new(physical_device.clone(), vendor_private_data)?;
        Ok(Arc::new(device))
    }
}

impl WindowsPhysicalDevice for WddmAdapter {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        self.handle
    }

    fn segment_group_size(&self) -> D3DKMT_SEGMENTGROUPSIZEINFO {
        self.segment_group_size
    }
}

impl AsVirtGpu for WddmAdapter {}
impl PhysicalDevice for WddmAdapter {}

impl Drop for WddmAdapter {
    fn drop(&mut self) {
        let mut close = D3DKMT_CLOSEADAPTER {
            hAdapter: self.handle,
        };
        // SAFETY: Safe since we own the adapter handle
        log_ntstatus!(unsafe { D3DKMTCloseAdapter(&mut close as *mut D3DKMT_CLOSEADAPTER) });
    }
}

pub fn enumerate_adapters() -> MesaResult<Vec<(WddmAdapter, MagmaPciInfo, MagmaPciBusInfo)>> {
    let mut enum_adapters = D3DKMT_ENUMADAPTERS2::default();

    // SAFETY:
    //  - `enum_adapters` is stack-allocated and properly typed.
    //  - D3DKMTEnumAdapters2 does not modify any other memory.
    check_ntstatus!(unsafe {
        D3DKMTEnumAdapters2(&mut enum_adapters as *mut D3DKMT_ENUMADAPTERS2)
    })?;

    // First call gets enum_adapters.NumAdapters, second call gets the actual data.
    let mut adapter_slice = vec![D3DKMT_ADAPTERINFO::default(); enum_adapters.NumAdapters as usize];
    enum_adapters.pAdapters = adapter_slice.as_mut_ptr();

    // SAFETY:
    //  - `enum_adapters` is stack-allocated and properly typed.
    //  - D3DKMTEnumAdapters2 does not modify any other memory.
    check_ntstatus!(unsafe {
        D3DKMTEnumAdapters2(&mut enum_adapters as *mut D3DKMT_ENUMADAPTERS2)
    })?;

    // Should not return a larger value of NumAdapters than it returned on the first call.
    assert!((enum_adapters.NumAdapters as usize) <= adapter_slice.len());
    let mut adapters = Vec::with_capacity(enum_adapters.NumAdapters as usize);

    for adapter in &mut adapter_slice[..(enum_adapters.NumAdapters as usize)] {
        let mut adapter = WddmAdapter::new(adapter.hAdapter, adapter.AdapterLuid);
        let (pci_info, pci_bus_info) = adapter.initialize()?;
        adapters.push((adapter, pci_info, pci_bus_info));
    }

    Ok(adapters)
}

impl WddmDevice {
    pub fn new(
        adapter: Arc<dyn PhysicalDevice>,
        vendor_private_data: Box<dyn VendorPrivateData>,
    ) -> MesaResult<WddmDevice> {
        let mut mem_props: MagmaMemoryProperties = Default::default();

        let mut arg = D3DKMT_CREATEDEVICE {
            Flags: Default::default(),
            Anonymous: D3DKMT_CREATEDEVICE_0 {
                hAdapter: adapter.as_wddm_handle(),
            },
            ..Default::default()
        };

        // Safe because mutable arg is allocated locally on the stack and we trust the D3DKMT API
        // not to modify any other memory.
        check_ntstatus!(unsafe { D3DKMTCreateDevice(&mut arg as *mut D3DKMT_CREATEDEVICE) })?;

        let segment_group_size = adapter.segment_group_size();
        if segment_group_size.NonLocalMemory > 0 {
            mem_props.add_heap(segment_group_size.NonLocalMemory, 0);
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            );
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            );
            mem_props.increment_heap_count();
        }

        if segment_group_size.LocalMemory > 0 {
            mem_props.add_heap(segment_group_size.LocalMemory, MAGMA_HEAP_DEVICE_LOCAL_BIT);
            mem_props.add_memory_type(MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
            mem_props.increment_heap_count();
        }

        Ok(WddmDevice {
            handle: arg.hDevice,
            adapter,
            vendor_private_data,
            mem_props,
        })
    }
}

impl GenericDevice for WddmDevice {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties> {
        Ok(self.mem_props.clone())
    }

    fn get_memory_budget(&self, heap_idx: u32) -> MesaResult<MagmaHeapBudget> {
        if heap_idx >= self.mem_props.memory_heap_count {
            return Err(MesaError::WithContext("Heap Index out of bounds"));
        }

        let mut segment_group = D3DKMT_MEMORY_SEGMENT_GROUP_NON_LOCAL;
        if self.mem_props.get_memory_heap(heap_idx).is_device_local() {
            segment_group = D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL;
        }

        let mut arg = D3DKMT_QUERYVIDEOMEMORYINFO {
            hProcess: std::ptr::null_mut::<c_void>(),
            hAdapter: self.adapter.as_wddm_handle(),
            MemorySegmentGroup: segment_group,
            Budget: 0,                  // output
            CurrentUsage: 0,            // output
            CurrentReservation: 0,      // output
            AvailableForReservation: 0, // output
            PhysicalAdapterIndex: 0,
        };

        check_ntstatus!(unsafe {
            D3DKMTQueryVideoMemoryInfo(&mut arg as *mut D3DKMT_QUERYVIDEOMEMORYINFO)
        })?;

        Ok(MagmaHeapBudget {
            budget: arg.Budget,
            usage: arg.CurrentUsage,
        })
    }

    fn create_context(&self, device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>> {
        let ctx = WddmContext::new(device.clone())?;
        Ok(Arc::new(ctx))
    }

    fn create_buffer(
        &self,
        device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let buf = WddmBuffer::new(device.clone(), create_info, &self.mem_props)?;
        Ok(Arc::new(buf))
    }

    fn import(
        &self,
        device: &Arc<dyn Device>,
        info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let mut open_alloc_info: D3DDDI_OPENALLOCATIONINFO2 = Default::default();

        let mut arg = D3DKMT_OPENRESOURCEFROMNTHANDLE {
            hDevice: self.handle,
            hNtHandle: info.handle.os_handle.into_raw_descriptor(),
            NumAllocations: 1,
            pOpenAllocationInfo2: &mut open_alloc_info as *mut _,
            PrivateRuntimeDataSize: 0,
            pPrivateRuntimeData: std::ptr::null_mut(),
            hResource: 0, // output
            KeyedMutexPrivateRuntimeDataSize: 0,
            pKeyedMutexPrivateRuntimeData: std::ptr::null_mut(),
            ResourcePrivateDriverDataSize: 0,
            pResourcePrivateDriverData: std::ptr::null_mut(),
            TotalPrivateDriverDataBufferSize: 0,
            pTotalPrivateDriverDataBuffer: std::ptr::null_mut(),
            hKeyedMutex: 0,
            hSyncObject: 0,
        };

        check_ntstatus!(unsafe { D3DKMTOpenResourceFromNtHandle(&mut arg) })?;

        let buf =
            WddmBuffer::from_existing(device.clone(), open_alloc_info.hAllocation, info.size)?;
        Ok(Arc::new(buf))
    }
}

impl Drop for WddmDevice {
    fn drop(&mut self) {
        let arg = D3DKMT_DESTROYDEVICE {
            hDevice: self.handle,
        };

        // Safe because const arg is allocated locally on the stack and we trust the D3DKMT API
        // not to modify any other memory.
        log_ntstatus!(unsafe { D3DKMTDestroyDevice(&arg as *const D3DKMT_DESTROYDEVICE) })
    }
}

impl WindowsDevice for WddmDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        self.handle
    }

    fn vendor_private_data(&self) -> Option<&dyn VendorPrivateData> {
        Some(&*self.vendor_private_data)
    }
}

impl Device for WddmDevice {}

impl WddmContext {
    pub fn new(device: Arc<dyn Device>) -> MesaResult<WddmContext> {
        // TODO: Fill in NodeOrdinal, EngineAffinity, pPrivateDriverData
        let mut arg = D3DKMT_CREATECONTEXTVIRTUAL {
            hDevice: device.as_wddm_handle(),
            NodeOrdinal: Default::default(),
            EngineAffinity: Default::default(),
            Flags: D3DDDI_CREATECONTEXTFLAGS {
                Anonymous: D3DDDI_CREATECONTEXTFLAGS_0 {
                    Value: Default::default(),
                },
            },
            pPrivateDriverData: std::ptr::null_mut::<c_void>(),
            PrivateDriverDataSize: Default::default(),
            ClientHint: D3DKMT_CLIENTHINT_VULKAN,
            hContext: 0, // return value
        };

        check_ntstatus!(unsafe {
            D3DKMTCreateContextVirtual(&mut arg as *mut D3DKMT_CREATECONTEXTVIRTUAL)
        })?;

        Ok(WddmContext {
            handle: arg.hContext,
            _device: device,
        })
    }
}

impl Drop for WddmContext {
    fn drop(&mut self) {
        // Safe because const arg is allocated locally on the stack and we trust the D3DKMT API
        // not to modify any other memory.
        log_ntstatus!(unsafe {
            D3DKMTDestroyContext(&D3DKMT_DESTROYCONTEXT {
                hContext: self.handle,
            } as *const D3DKMT_DESTROYCONTEXT)
        })
    }
}

impl Context for WddmContext {}

impl WddmBuffer {
    pub fn new(
        device: Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
        mem_props: &MagmaMemoryProperties,
    ) -> MesaResult<WddmBuffer> {
        let vendor_private_data = device.vendor_private_data().unwrap();

        let flags: D3DKMT_CREATEALLOCATIONFLAGS = Default::default();

        // flags.set_NonSecure(1);
        // flags.set_CreateWriteCombined(1);

        // type annotations important for following calculation
        let mut create_allocation: Vec<u32> = vendor_private_data.createallocation_pdata();
        let mut allocationinfo2: Vec<u32> =
            vendor_private_data.allocationinfo2_pdata(create_info, mem_props);

        let size_create_allocation: usize = create_allocation.len() * std::mem::size_of::<u32>();
        let size_allocationinfo2: usize = allocationinfo2.len() * std::mem::size_of::<u32>();

        let mut alloc_info: D3DDDI_ALLOCATIONINFO2 = D3DDDI_ALLOCATIONINFO2 {
            pPrivateDriverData: allocationinfo2.as_mut_ptr() as *mut c_void,
            PrivateDriverDataSize: size_allocationinfo2.try_into()?,
            ..Default::default()
        };

        let mut arg = D3DKMT_CREATEALLOCATION {
            hDevice: device.as_wddm_handle(),
            hResource: Default::default(),
            hGlobalShare: 0,
            pPrivateRuntimeData: std::ptr::null_mut::<c_void>(),
            PrivateRuntimeDataSize: 0,
            PrivateDriverDataSize: size_create_allocation.try_into()?,
            NumAllocations: 1,
            Anonymous1: D3DKMT_CREATEALLOCATION_0 {
                pPrivateDriverData: create_allocation.as_mut_ptr() as *mut c_void,
            },
            Anonymous2: D3DKMT_CREATEALLOCATION_1 {
                pAllocationInfo2: &mut alloc_info as *mut D3DDDI_ALLOCATIONINFO2,
            },
            Flags: flags,
            hPrivateRuntimeResourceHandle: std::ptr::null_mut::<c_void>(), // output of D3DKMTCreateAllocation
        };

        check_ntstatus!(unsafe {
            D3DKMTCreateAllocation2(&mut arg as *mut D3DKMT_CREATEALLOCATION)
        })?;

        Ok(WddmBuffer {
            handle: alloc_info.hAllocation,
            device,
            size: create_info.size,
        })
    }
    pub fn from_existing(
        device: Arc<dyn Device>,
        handle: D3dkmtHandle,
        size: u64,
    ) -> MesaResult<WddmBuffer> {
        Ok(WddmBuffer {
            handle,
            device,
            size,
        })
    }
}

unsafe impl Send for WddmMapping {}
unsafe impl Sync for WddmMapping {}

unsafe impl MappedRegion for WddmMapping {
    fn as_ptr(&self) -> *mut u8 {
        self.pdata as *mut u8
    }

    fn size(&self) -> usize {
        self.size
    }

    fn as_mesa_mapping(&self) -> MesaMapping {
        MesaMapping {
            ptr: self.pdata as u64,
            size: self.size as u64,
        }
    }
}

impl GenericBuffer for WddmBuffer {
    fn map(&self, buffer: &Arc<dyn Buffer>) -> MesaResult<Arc<dyn MappedRegion>> {
        let mut arg = D3DKMT_LOCK2 {
            hDevice: self.device.as_wddm_handle(),
            hAllocation: self.handle,
            ..Default::default()
        };

        check_ntstatus!(unsafe { D3DKMTLock2(&mut arg as *mut D3DKMT_LOCK2) })?;

        Ok(Arc::new(WddmMapping {
            _buffer: buffer.clone(),
            pdata: arg.pData,
            size: self.size.try_into()?,
        }))
    }

    fn export(&self) -> MesaResult<MesaHandle> {
        Err(MesaError::Unsupported)
    }

    fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        let mut arg = D3DKMT_INVALIDATECACHE {
            hDevice: self.device.as_wddm_handle(),
            hAllocation: self.handle,
            ..Default::default()
        };

        if (sync_flags & MAGMA_SYNC_WHOLE_RANGE) != 0 {
            arg.Offset = 0;
            arg.Length = self.size.try_into()?;
            check_ntstatus!(unsafe {
                D3DKMTInvalidateCache(&mut arg as *mut D3DKMT_INVALIDATECACHE)
            })?;
        } else if (sync_flags & MAGMA_SYNC_RANGES) != 0 {
            for r in ranges {
                arg.Offset = r.offset.try_into()?;
                arg.Length = r.size.try_into()?;
                check_ntstatus!(unsafe {
                    D3DKMTInvalidateCache(&mut arg as *mut D3DKMT_INVALIDATECACHE)
                })?;
            }
        }
        Ok(())
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        Ok(())
    }
}

impl Drop for WddmBuffer {
    fn drop(&mut self) {
        // Safe because const arg is allocated locally on the stack and we trust the D3DKMT API
        // not to modify any other memory.
        let arg = D3DKMT_DESTROYALLOCATION2 {
            hDevice: self.device.as_wddm_handle(),
            hResource: Default::default(),
            phAllocationList: &self.handle as *const D3dkmtHandle,
            AllocationCount: 1,
            Flags: D3DDDICB_DESTROYALLOCATION2FLAGS {
                Anonymous: D3DDDICB_DESTROYALLOCATION2FLAGS_0 {
                    Value: Default::default(),
                },
            },
        };

        log_ntstatus!(unsafe { D3DKMTDestroyAllocation2(&arg as *const D3DKMT_DESTROYALLOCATION2) })
    }
}

impl Buffer for WddmBuffer {}

unsafe impl Send for WddmDevice {}
unsafe impl Sync for WddmDevice {}

unsafe impl Send for WddmContext {}
unsafe impl Sync for WddmContext {}

unsafe impl Send for WddmBuffer {}
unsafe impl Sync for WddmBuffer {}
