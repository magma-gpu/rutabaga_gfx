// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::fd::BorrowedFd;
use std::path::Path;
use std::sync::Arc;

use libc::O_CLOEXEC;
use libc::O_RDWR;
use log::error;
use magma_gpu::log_status;
use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MemoryMapping;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::RawDescriptor;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF;
use rustix::fs::major;
use rustix::fs::minor;
use rustix::fs::open;
use rustix::fs::readlink;
use rustix::fs::stat;
use rustix::fs::Dir;
use rustix::fs::Mode;
use rustix::fs::OFlags;
use zerocopy::TryFromBytes;

use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaVendorId;
use crate::defines::MAGMA_BUS_TYPE_PCI;
use crate::defines::MAGMA_BUS_TYPE_PLATFORM;
use crate::device::PhysicalDevice;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::bindings::drm_bindings::drm_gem_close;
use crate::sys::linux::bindings::drm_bindings::drm_prime_handle;
use crate::sys::linux::drm_ioctl_gem_close;
use crate::sys::linux::drm_ioctl_prime_fd_to_handle;
use crate::sys::linux::drm_ioctl_prime_handle_to_fd;
use crate::sys::linux::get_drm_device_name;
use crate::sys::linux::virtgpu_enumerate_devices;
use crate::sys::linux::AmdGpuPhysicalDevice;
use crate::sys::linux::I915PhysicalDevice;
use crate::sys::linux::KgslPhysicalDevice;
use crate::sys::linux::MsmPhysicalDevice;
use crate::sys::linux::PanthorPhysicalDevice;
use crate::sys::linux::XePhysicalDevice;
use crate::sys::linux::DRM_DIR_NAME;
use crate::sys::linux::DRM_RENDER_MINOR_NAME;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericPhysicalDevice;

const PCI_ATTRS: [&str; 5] = [
    "revision",
    "vendor",
    "device",
    "subsystem_vendor",
    "subsystem_device",
];

#[allow(dead_code)]
pub trait PlatformPhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        None
    }

    fn as_fd(&self) -> Option<BorrowedFd<'_>> {
        self.as_descriptor().map(|d| d.as_fd())
    }

    fn as_raw_descriptor(&self) -> RawDescriptor {
        self.as_descriptor()
            .map(|d| d.as_raw_descriptor())
            .unwrap_or(-1)
    }

    fn cpu_map(&self, offset: u64, size: usize) -> Result<MemoryMapping> {
        let desc = self.as_descriptor().ok_or(Error::Unimplemented)?;
        Ok(MemoryMapping::from_offset(desc, offset.try_into()?, size)?)
    }

    fn export(&self, gem_handle: u32) -> Result<MagmaGpuHandle> {
        let fd = self.as_fd().unwrap();
        let mut arg: drm_prime_handle = drm_prime_handle {
            handle: gem_handle,
            flags: (O_CLOEXEC | O_RDWR) as u32,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_prime_handle
        let prime_fd = unsafe {
            drm_ioctl_prime_handle_to_fd(fd, &mut arg)?;
            arg.fd
        };

        // SAFETY:
        // `prime_fd` is valid after a successful PRIME_HANDLE_TO_HANDLE syscall.
        let descriptor = unsafe { OwnedDescriptor::from_raw_descriptor(prime_fd) };

        Ok(MagmaGpuHandle {
            os_handle: descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF,
        })
    }

    fn import(&self, handle: MagmaGpuHandle) -> Result<u32> {
        let fd = self.as_fd().unwrap();
        let mut arg: drm_prime_handle = drm_prime_handle {
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_prime_handle
        let handle = unsafe {
            arg.fd = handle.os_handle.as_raw_descriptor();
            drm_ioctl_prime_fd_to_handle(fd, &mut arg)?;
            arg.handle
        };

        Ok(handle)
    }

    fn close(&self, gem_handle: u32) {
        let fd = self.as_fd().unwrap();
        let arg: drm_gem_close = drm_gem_close {
            handle: gem_handle,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_gem_handle
        let result = unsafe { drm_ioctl_gem_close(fd, &arg) };

        log_status!(result);
    }
}

pub trait PlatformDevice {}

// Helper function to parse hexadecimal string to u16
fn parse_hex_u16(s: &str) -> Result<u16> {
    let valid_str = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    Ok(u16::from_str_radix(valid_str, 16)?)
}

fn enumerate_drm_devices() -> Result<Vec<PhysicalDevice>> {
    let mut devices: Vec<PhysicalDevice> = Vec::new();
    let dir_fd = match open(
        DRM_DIR_NAME,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(_) => return Ok(devices),
    };

    let dir = Dir::new(dir_fd)?;
    for entry in dir.flatten() {
        let filename = entry.file_name().to_str()?;
        if filename.contains(DRM_RENDER_MINOR_NAME) {
            let path = Path::new(DRM_DIR_NAME).join(filename);
            let statbuf = stat(&path)?;

            let maj = major(statbuf.st_rdev);
            let min = minor(statbuf.st_rdev);

            let pci_device_dir = format!("/sys/dev/char/{maj}:{min}/device");
            let pci_subsystem_dir = format!("{pci_device_dir}/subsystem");
            let subsystem_path = Path::new(&pci_subsystem_dir);
            let subsystem = readlink(subsystem_path, Vec::new())?;

            // If not valid UTF-8, assume not PCI
            let is_pci_subsystem = subsystem
                .to_str()
                .map(|s| s.contains("/pci"))
                .unwrap_or(false);

            if !is_pci_subsystem {
                continue;
            }

            let mut info = MagmaPhysicalDeviceInfo {
                bus_type: MAGMA_BUS_TYPE_PCI,
                ..Default::default()
            };
            for attr in PCI_ATTRS {
                let attr_path = format!("{pci_device_dir}/{attr}");
                let mut file = File::open(attr_path)?;
                let mut hex_string = String::new();
                file.read_to_string(&mut hex_string)?;

                match attr {
                    "revision" => {
                        info.pci_bus_info.revision_id = parse_hex_u16(&hex_string)?.try_into()?
                    }
                    "vendor" => {
                        let vendor_u16 = parse_hex_u16(&hex_string)?;
                        info.vendor_id =
                            TryFromBytes::try_read_from_bytes(&vendor_u16.to_ne_bytes())
                                .map_err(|_| Error::Unimplemented)?;
                    }
                    "device" => info.device_id = parse_hex_u16(&hex_string)?,
                    "subsystem_vendor" => {
                        info.pci_bus_info.subvendor_id = parse_hex_u16(&hex_string)?
                    }
                    "subsystem_device" => {
                        info.pci_bus_info.subdevice_id = parse_hex_u16(&hex_string)?
                    }
                    _ => {}
                }
            }

            let uevent_path = format!("{pci_device_dir}/uevent");
            if let Ok(text) = std::fs::read_to_string(uevent_path) {
                for line in text.lines() {
                    if line.contains("PCI_SLOT_NAME") {
                        let v: Vec<&str> = line.split(&['=', ':', '.'][..]).collect();
                        if v.len() >= 5 {
                            info.pci_bus_info.domain = v[1].parse::<u16>()?;
                            info.pci_bus_info.bus = v[2].parse::<u8>()?;
                            info.pci_bus_info.device = v[3].parse::<u8>()?;
                            info.pci_bus_info.function = v[4].parse::<u8>()?;
                        }
                    }
                }
            }

            let descriptor: OwnedDescriptor = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path.clone())?
                .into();

            let name = get_drm_device_name(&descriptor)?;
            let physical_devices: Vec<Arc<dyn BackendPhysicalDevice>> = match name.as_str() {
                "amdgpu" => vec![Arc::new(AmdGpuPhysicalDevice::new(descriptor))],
                "xe" => vec![Arc::new(XePhysicalDevice::new(descriptor))],
                "i915" => vec![Arc::new(I915PhysicalDevice::new(descriptor))],
                "msm" => vec![Arc::new(MsmPhysicalDevice::new(descriptor))],
                "panthor" => vec![Arc::new(PanthorPhysicalDevice::new(descriptor))],
                "virtio_gpu" => virtgpu_enumerate_devices(descriptor, &path)?,
                _ => return Err(Error::Unimplemented),
            };

            for physical_device in physical_devices {
                let mut info = info;
                physical_device.get_device_info(&mut info)?;
                let queue_families = physical_device.get_queue_family_properties()?;
                info.queue_family_count =
                    queue_families.len().min(info.queue_families.len()) as u32;
                for (i, qf) in queue_families.iter().enumerate() {
                    if i >= info.queue_families.len() {
                        break;
                    }
                    info.queue_families[i] = *qf;
                }

                devices.push(PhysicalDevice::new(physical_device, info));
            }
        }
    }

    Ok(devices)
}

fn enumerate_downstream_devices() -> Result<Vec<PhysicalDevice>> {
    let mut devices: Vec<PhysicalDevice> = Vec::new();

    for kgsl_node in ["/dev/kgsl-3d0", "/dev/kgsl"] {
        let path = Path::new(kgsl_node);
        if path.exists() {
            if let Ok(file) = OpenOptions::new().read(true).write(true).open(path) {
                let descriptor: OwnedDescriptor = file.into();
                let kgsl_device = Arc::new(KgslPhysicalDevice::new(descriptor));
                let mut info = MagmaPhysicalDeviceInfo {
                    bus_type: MAGMA_BUS_TYPE_PLATFORM,
                    vendor_id: MagmaVendorId::Qualcomm,
                    device_id: kgsl_device.gpu_id() as u16,
                    ..Default::default()
                };
                if let Ok(queue_families) = kgsl_device.get_queue_family_properties() {
                    info.queue_family_count =
                        queue_families.len().min(info.queue_families.len()) as u32;
                    for (i, qf) in queue_families.iter().enumerate() {
                        if i >= info.queue_families.len() {
                            break;
                        }
                        info.queue_families[i] = *qf;
                    }
                }
                let physical_device: Arc<dyn BackendPhysicalDevice> = kgsl_device;
                devices.push(PhysicalDevice::new(physical_device, info));
                break;
            }
        }
    }

    Ok(devices)
}

pub fn enumerate_devices() -> Result<Vec<PhysicalDevice>> {
    let mut devices = enumerate_drm_devices()?;
    let mut downstream = enumerate_downstream_devices()?;
    devices.append(&mut downstream);
    Ok(devices)
}
