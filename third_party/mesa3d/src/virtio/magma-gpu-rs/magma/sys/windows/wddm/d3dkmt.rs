// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::os::raw::c_void;
use std::ptr::null_mut;
use std::slice::from_raw_parts;

use libc::wcslen;
use log::error;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::IntoRawDescriptor;

use windows_sys::Wdk::Graphics::Direct3D::*;
use zerocopy::TryFromBytes;

use crate::check_ntstatus;
use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaHeapBudget;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MAGMA_BUS_TYPE_PCI;
use crate::defines::MAGMA_SYNC_RANGES;
use crate::defines::MAGMA_SYNC_WHOLE_RANGE;
use crate::error::Error;
use crate::error::Result;
use crate::log_ntstatus;
use crate::sys::windows::VendorPrivateData;

pub type D3dkmtHandle = u32;

pub struct AdapterQueryResult {
    pub info: MagmaPhysicalDeviceInfo,
    pub segment_group_size: D3DKMT_SEGMENTGROUPSIZEINFO,
    pub adapter_name: String,
    pub chip_type: String,
}

pub fn query_adapter(handle: D3dkmtHandle) -> Result<AdapterQueryResult> {
    let mut info = MagmaPhysicalDeviceInfo {
        bus_type: MAGMA_BUS_TYPE_PCI,
        ..Default::default()
    };

    let mut query_device_ids: D3DKMT_QUERY_DEVICE_IDS = Default::default();
    let mut adapter_address: D3DKMT_ADAPTERADDRESS = Default::default();
    let mut segment_group_size: D3DKMT_SEGMENTGROUPSIZEINFO = Default::default();

    let mut adapter_info = D3DKMT_QUERYADAPTERINFO {
        hAdapter: handle,
        Type: KMTQAITYPE_PHYSICALADAPTERDEVICEIDS,
        pPrivateDriverData: &mut query_device_ids as *mut D3DKMT_QUERY_DEVICE_IDS as *mut c_void,
        PrivateDriverDataSize: size_of::<D3DKMT_QUERY_DEVICE_IDS>() as u32,
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
    adapter_info.PrivateDriverDataSize = size_of::<D3DKMT_ADAPTERADDRESS>() as u32;

    // SAFETY:
    //  - `adapter_info` is stack-allocated and properly typed.
    //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
    //      KMTQAITYPE_ADAPTERADDRESS operation
    check_ntstatus!(unsafe {
        D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
    })?;

    let mut wddm_caps: D3DKMT_WDDM_2_7_CAPS = Default::default();
    adapter_info.Type = KMTQAITYPE_WDDM_2_7_CAPS;
    adapter_info.pPrivateDriverData = &mut wddm_caps as *mut D3DKMT_WDDM_2_7_CAPS as *mut c_void;
    adapter_info.PrivateDriverDataSize = size_of::<D3DKMT_WDDM_2_7_CAPS>() as u32;

    // SAFETY:
    //  - `adapter_info` is stack-allocated and properly typed.
    //  - `pPrivateDriverData` and `PrivateDriverDataSize` are both correct for the
    //      KMTQAITYPE_WDDM_2_7_CAPS operation
    check_ntstatus!(unsafe {
        D3DKMTQueryAdapterInfo(&mut adapter_info as *mut D3DKMT_QUERYADAPTERINFO)
    })?;

    adapter_info.Type = KMTQAITYPE_GETSEGMENTGROUPSIZE;
    adapter_info.pPrivateDriverData =
        &mut segment_group_size as *mut D3DKMT_SEGMENTGROUPSIZEINFO as *mut c_void;
    adapter_info.PrivateDriverDataSize = size_of::<D3DKMT_SEGMENTGROUPSIZEINFO>() as u32;

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
    adapter_info.PrivateDriverDataSize = size_of::<D3DKMT_ADAPTERREGISTRYINFO>() as u32;

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

    let adapter_name = String::from_utf16(adapter_name_slice).map_err(|_| Error::InvalidArgs)?;
    let chip_type = String::from_utf16(chip_type_slice).map_err(|_| Error::InvalidArgs)?;

    let device_ids = query_device_ids.DeviceIds;
    let vendor_u16: u16 = device_ids.VendorID.try_into()?;
    info.vendor_id = TryFromBytes::try_read_from_bytes(&vendor_u16.to_ne_bytes())
        .map_err(|_| Error::Unimplemented)?;
    info.device_id = device_ids.DeviceID.try_into()?;

    Ok(AdapterQueryResult {
        info,
        segment_group_size,
        adapter_name,
        chip_type,
    })
}

pub fn close_adapter(handle: D3dkmtHandle) {
    let mut close = D3DKMT_CLOSEADAPTER { hAdapter: handle };
    // SAFETY: Safe since we own the adapter handle
    log_ntstatus!(unsafe { D3DKMTCloseAdapter(&mut close as *mut D3DKMT_CLOSEADAPTER) });
}

pub fn enum_adapters() -> Result<Vec<D3DKMT_ADAPTERINFO>> {
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

    assert!((enum_adapters.NumAdapters as usize) <= adapter_slice.len());
    adapter_slice.truncate(enum_adapters.NumAdapters as usize);
    Ok(adapter_slice)
}

pub fn create_device(adapter_handle: D3dkmtHandle) -> Result<D3dkmtHandle> {
    let mut arg = D3DKMT_CREATEDEVICE {
        Flags: Default::default(),
        Anonymous: D3DKMT_CREATEDEVICE_0 {
            hAdapter: adapter_handle,
        },
        ..Default::default()
    };

    // SAFETY: Mutable arg is allocated locally on the stack.
    check_ntstatus!(unsafe { D3DKMTCreateDevice(&mut arg as *mut D3DKMT_CREATEDEVICE) })?;
    Ok(arg.hDevice)
}

pub fn destroy_device(device_handle: D3dkmtHandle) {
    let arg = D3DKMT_DESTROYDEVICE {
        hDevice: device_handle,
    };

    // SAFETY: Const arg is allocated locally on the stack.
    log_ntstatus!(unsafe { D3DKMTDestroyDevice(&arg as *const D3DKMT_DESTROYDEVICE) })
}

pub fn query_video_memory_info(
    adapter_handle: D3dkmtHandle,
    is_device_local: bool,
) -> Result<MagmaHeapBudget> {
    let segment_group = if is_device_local {
        D3DKMT_MEMORY_SEGMENT_GROUP_LOCAL
    } else {
        D3DKMT_MEMORY_SEGMENT_GROUP_NON_LOCAL
    };

    let mut arg = D3DKMT_QUERYVIDEOMEMORYINFO {
        hProcess: null_mut::<c_void>(),
        hAdapter: adapter_handle,
        MemorySegmentGroup: segment_group,
        Budget: 0,
        CurrentUsage: 0,
        CurrentReservation: 0,
        AvailableForReservation: 0,
        PhysicalAdapterIndex: 0,
    };

    // SAFETY: Valid D3DKMT_QUERYVIDEOMEMORYINFO struct.
    check_ntstatus!(unsafe {
        D3DKMTQueryVideoMemoryInfo(&mut arg as *mut D3DKMT_QUERYVIDEOMEMORYINFO)
    })?;

    Ok(MagmaHeapBudget {
        budget: arg.Budget,
        usage: arg.CurrentUsage,
    })
}

pub fn open_resource_from_nt_handle(
    device_handle: D3dkmtHandle,
    handle: MagmaGpuHandle,
) -> Result<D3dkmtHandle> {
    let mut open_alloc_info: D3DDDI_OPENALLOCATIONINFO2 = Default::default();

    let mut arg = D3DKMT_OPENRESOURCEFROMNTHANDLE {
        hDevice: device_handle,
        hNtHandle: handle.os_handle.into_raw_descriptor(),
        NumAllocations: 1,
        pOpenAllocationInfo2: &mut open_alloc_info as *mut _,
        PrivateRuntimeDataSize: 0,
        pPrivateRuntimeData: null_mut(),
        hResource: 0,
        KeyedMutexPrivateRuntimeDataSize: 0,
        pKeyedMutexPrivateRuntimeData: null_mut(),
        ResourcePrivateDriverDataSize: 0,
        pResourcePrivateDriverData: null_mut(),
        TotalPrivateDriverDataBufferSize: 0,
        pTotalPrivateDriverDataBuffer: null_mut(),
        hKeyedMutex: 0,
        hSyncObject: 0,
    };

    // SAFETY: Valid D3DKMT_OPENRESOURCEFROMNTHANDLE struct.
    check_ntstatus!(unsafe { D3DKMTOpenResourceFromNtHandle(&mut arg) })?;
    Ok(open_alloc_info.hAllocation)
}

pub fn create_context_virtual(device_handle: D3dkmtHandle) -> Result<D3dkmtHandle> {
    let mut arg = D3DKMT_CREATECONTEXTVIRTUAL {
        hDevice: device_handle,
        NodeOrdinal: Default::default(),
        EngineAffinity: Default::default(),
        Flags: D3DDDI_CREATECONTEXTFLAGS {
            Anonymous: D3DDDI_CREATECONTEXTFLAGS_0 {
                Value: Default::default(),
            },
        },
        pPrivateDriverData: null_mut::<c_void>(),
        PrivateDriverDataSize: Default::default(),
        ClientHint: D3DKMT_CLIENTHINT_VULKAN,
        hContext: 0,
    };

    // SAFETY: Valid D3DKMT_CREATECONTEXTVIRTUAL struct.
    check_ntstatus!(unsafe {
        D3DKMTCreateContextVirtual(&mut arg as *mut D3DKMT_CREATECONTEXTVIRTUAL)
    })?;

    Ok(arg.hContext)
}

pub fn destroy_context(context_handle: D3dkmtHandle) {
    // SAFETY: Const arg is allocated locally on the stack.
    log_ntstatus!(unsafe {
        D3DKMTDestroyContext(&D3DKMT_DESTROYCONTEXT {
            hContext: context_handle,
        } as *const D3DKMT_DESTROYCONTEXT)
    })
}

pub fn create_allocation(
    device_handle: D3dkmtHandle,
    vendor_private_data: &dyn VendorPrivateData,
    create_info: &MagmaCreateBufferInfo,
    mem_types: &[MagmaMemoryType],
) -> Result<D3dkmtHandle> {
    let flags: D3DKMT_CREATEALLOCATIONFLAGS = Default::default();
    let mut create_allocation: Vec<u32> = vendor_private_data.createallocation_pdata();
    let mut allocationinfo2: Vec<u32> =
        vendor_private_data.allocationinfo2_pdata(create_info, mem_types);

    let size_create_allocation: usize = create_allocation.len() * size_of::<u32>();
    let size_allocationinfo2: usize = allocationinfo2.len() * size_of::<u32>();

    let mut alloc_info: D3DDDI_ALLOCATIONINFO2 = D3DDDI_ALLOCATIONINFO2 {
        pPrivateDriverData: allocationinfo2.as_mut_ptr() as *mut c_void,
        PrivateDriverDataSize: size_allocationinfo2.try_into()?,
        ..Default::default()
    };

    let mut arg = D3DKMT_CREATEALLOCATION {
        hDevice: device_handle,
        hResource: Default::default(),
        hGlobalShare: 0,
        pPrivateRuntimeData: null_mut::<c_void>(),
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
        hPrivateRuntimeResourceHandle: null_mut::<c_void>(),
    };

    // SAFETY: Valid D3DKMT_CREATEALLOCATION struct.
    check_ntstatus!(unsafe { D3DKMTCreateAllocation2(&mut arg as *mut D3DKMT_CREATEALLOCATION) })?;

    Ok(alloc_info.hAllocation)
}

pub fn lock_allocation(
    device_handle: D3dkmtHandle,
    alloc_handle: D3dkmtHandle,
) -> Result<*mut c_void> {
    let mut arg = D3DKMT_LOCK2 {
        hDevice: device_handle,
        hAllocation: alloc_handle,
        ..Default::default()
    };

    // SAFETY: Valid D3DKMT_LOCK2 struct.
    check_ntstatus!(unsafe { D3DKMTLock2(&mut arg as *mut D3DKMT_LOCK2) })?;
    Ok(arg.pData)
}

pub fn invalidate_cache(
    device_handle: D3dkmtHandle,
    alloc_handle: D3dkmtHandle,
    total_size: u64,
    sync_flags: u64,
    ranges: &[MagmaMappedMemoryRange],
) -> Result<()> {
    let mut arg = D3DKMT_INVALIDATECACHE {
        hDevice: device_handle,
        hAllocation: alloc_handle,
        ..Default::default()
    };

    if (sync_flags & MAGMA_SYNC_WHOLE_RANGE) != 0 {
        arg.Offset = 0;
        arg.Length = total_size.try_into()?;
        // SAFETY: Valid D3DKMT_INVALIDATECACHE struct.
        check_ntstatus!(unsafe { D3DKMTInvalidateCache(&mut arg as *mut D3DKMT_INVALIDATECACHE) })?;
    } else if (sync_flags & MAGMA_SYNC_RANGES) != 0 {
        for r in ranges {
            arg.Offset = r.offset.try_into()?;
            arg.Length = r.size.try_into()?;
            // SAFETY: Valid D3DKMT_INVALIDATECACHE struct.
            check_ntstatus!(unsafe {
                D3DKMTInvalidateCache(&mut arg as *mut D3DKMT_INVALIDATECACHE)
            })?;
        }
    }
    Ok(())
}

pub fn destroy_allocation(device_handle: D3dkmtHandle, alloc_handle: D3dkmtHandle) {
    let arg = D3DKMT_DESTROYALLOCATION2 {
        hDevice: device_handle,
        hResource: Default::default(),
        phAllocationList: &alloc_handle as *const D3dkmtHandle,
        AllocationCount: 1,
        Flags: D3DDDICB_DESTROYALLOCATION2FLAGS {
            Anonymous: D3DDDICB_DESTROYALLOCATION2FLAGS_0 {
                Value: Default::default(),
            },
        },
    };

    // SAFETY: Const arg is allocated locally on the stack.
    log_ntstatus!(unsafe { D3DKMTDestroyAllocation2(&arg as *const D3DKMT_DESTROYALLOCATION2) })
}
