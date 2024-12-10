// Copyright 2025 Google
// SPDX-License-Identifier: MIT

// Taken from gfxstrand@'s WDDM branch, which seems reversed engineered.
//
// https://gitlab.freedesktop.org/gfxstrand/mesa/-/tree/radv/wddm2?ref_type=heads

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaMemoryProperties;
use crate::sys::windows::VendorPrivateData;

static AMD_CREATE_ALLOC_PDATA: [u32; 15] = [
    0x00000000, 0x00000000, 0x00000080, 0x00000420, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
];

static AMD_ALLOC_PDATA: [u32; 206] = [
    0x000002f8, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x0000036f, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000001,
    0x000002f8, 0x0000000d, 0x00000080, 0x00000000, 0xa0002008, 0x00270000, 0x00010000, 0x00001805,
    0x00000004, 0x00000004, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x01080000, 0x00270000, 0x00000000, 0x00000000, 0x00000000, 0x00270000, 0x00000000,
    0x00000000, 0x00000000, 0x00000001, 0x00000000, 0x00400000, 0x00000001, 0x00000001, 0x00000001,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x000001e4, 0x00000000, 0x00000000, 0x00000008,
    0x00000000, 0x00000120, 0x00020000, 0x00000020, 0x00000001, 0x00000000, 0x00270000, 0x00000001,
    0x00270000, 0x00000001, 0x00000001, 0x00270000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00270000, 0x00000001, 0x00000001, 0x00270000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000114, 0x00000000, 0x00000000, 0x00000000, 0x00270000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
];

pub struct Amd(pub ());

impl VendorPrivateData for Amd {
    fn createallocation_pdata(&self) -> Vec<u32> {
        Vec::from(AMD_CREATE_ALLOC_PDATA)
    }

    fn allocationinfo2_pdata(
        &self,
        create_info: &MagmaCreateBufferInfo,
        mem_props: &MagmaMemoryProperties,
    ) -> Vec<u32> {
        let mut alloc_pdata = AMD_ALLOC_PDATA;
        let memory_type = mem_props.get_memory_type(create_info.memory_type_idx);

        // FIXME: gpu_info.pte_fragment_size, alignment
        // Need GPU topology crate
        let size: u32 = 0;
        let phys_size: u32 = 0;
        let phys_alignment: u32 = 0;

        alloc_pdata[21] = phys_size;
        alloc_pdata[22] = phys_alignment;
        alloc_pdata[42] = phys_size;
        alloc_pdata[46] = phys_size;
        alloc_pdata[70] = size;
        alloc_pdata[72] = phys_size;
        alloc_pdata[75] = phys_size;
        alloc_pdata[92] = size;
        alloc_pdata[95] = phys_size;
        alloc_pdata[141] = phys_size;

        // Some sort of flags field
        alloc_pdata[20] = 0x00002000;

        if memory_type.is_coherent() {
            alloc_pdata[20] |= 0xa0000000;
            alloc_pdata[41] |= 0x00080000;
        } else {
            alloc_pdata[20] |= 0x80000000;
        }

        // Write-back cached
        if memory_type.is_device_local() {
            alloc_pdata[20] |= 0x00000005;
            alloc_pdata[24] = 0x00030101;
            alloc_pdata[25] = 0x00000001;
        } else if memory_type.is_coherent() && !memory_type.is_cached() {
            // RADEON_FLAG_GTT_WC
            alloc_pdata[20] |= 0x00000004;
            alloc_pdata[24] = 3;
            alloc_pdata[25] = 3;
        } else {
            alloc_pdata[20] |= 0x00000008;
            alloc_pdata[24] = 4;
            alloc_pdata[25] = 4;
        }

        Vec::from(alloc_pdata)
    }
}
