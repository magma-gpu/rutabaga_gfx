// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::fd::BorrowedFd;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use log::error;
use mesa3d_util::log_status;
use mesa3d_util::AsRawDescriptor;
use mesa3d_util::FromRawDescriptor;
use mesa3d_util::MemoryMapping;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaResult;
use mesa3d_util::OwnedDescriptor;
use mesa3d_util::RawDescriptor;
use mesa3d_util::MESA_HANDLE_TYPE_MEM_DMABUF;

use rustix::fs::major;
use rustix::fs::minor;
use rustix::fs::open;
use rustix::fs::readlink;
use rustix::fs::stat;
use rustix::fs::Dir;
use rustix::fs::Mode;
use rustix::fs::OFlags;

use libc::O_CLOEXEC;
use libc::O_RDWR;

use crate::magma::MagmaPhysicalDevice;
use crate::magma_defines::MagmaPciBusInfo;
use crate::magma_defines::MagmaPciInfo;
use crate::magma_defines::MAGMA_VENDOR_ID_AMD;
use crate::magma_defines::MAGMA_VENDOR_ID_INTEL;
use crate::magma_defines::MAGMA_VENDOR_ID_QCOM;

use crate::sys::linux::bindings::drm_bindings::drm_gem_close;
use crate::sys::linux::bindings::drm_bindings::drm_prime_handle;
use crate::sys::linux::drm_ioctl_gem_close;
use crate::sys::linux::drm_ioctl_prime_fd_to_handle;
use crate::sys::linux::drm_ioctl_prime_handle_to_fd;
use crate::sys::linux::get_drm_device_name;
use crate::sys::linux::AmdGpu;
use crate::sys::linux::Msm;
use crate::sys::linux::Xe;
use crate::sys::linux::DRM_DIR_NAME;
use crate::sys::linux::DRM_RENDER_MINOR_NAME;
use crate::sys::linux::I915;

use crate::traits::AsVirtGpu;
use crate::traits::Device;
use crate::traits::GenericPhysicalDevice;
use crate::traits::PhysicalDevice;

const PCI_ATTRS: [&str; 5] = [
    "revision",
    "vendor",
    "device",
    "subsystem_vendor",
    "subsystem_device",
];

#[derive(Debug)]
pub struct LinuxPhysicalDevice {
    descriptor: OwnedDescriptor,
    name: String,
}

#[allow(dead_code)]
pub trait PlatformPhysicalDevice {
    fn as_fd(&self) -> Option<BorrowedFd<'_>> {
        None
    }

    fn as_raw_descriptor(&self) -> RawDescriptor {
        -1
    }

    fn cpu_map(&self, _offset: u64, _size: usize) -> MesaResult<MemoryMapping> {
        Err(MesaError::Unsupported)
    }

    fn export(&self, _gem_handle: u32) -> MesaResult<MesaHandle> {
        Err(MesaError::Unsupported)
    }

    fn import(&self, _handle: MesaHandle) -> MesaResult<u32> {
        Err(MesaError::Unsupported)
    }

    fn close(&self, _gem_handle: u32) {}
}

impl GenericPhysicalDevice for LinuxPhysicalDevice {
    fn create_device(
        &self,
        physical_device: &Arc<dyn PhysicalDevice>,
        pci_info: &MagmaPciInfo,
    ) -> MesaResult<Arc<dyn Device>> {
        let device: Arc<dyn Device> = match pci_info.vendor_id {
            MAGMA_VENDOR_ID_AMD => Arc::new(AmdGpu::new(physical_device.clone())?),
            MAGMA_VENDOR_ID_QCOM => Arc::new(Msm::new(physical_device.clone())),
            MAGMA_VENDOR_ID_INTEL => {
                if self.name == "xe" {
                    Arc::new(Xe::new(physical_device.clone(), pci_info)?)
                } else {
                    Arc::new(I915::new(physical_device.clone())?)
                }
            }
            _ => todo!(),
        };

        Ok(device)
    }
}

pub trait PlatformDevice {}

impl LinuxPhysicalDevice {
    pub fn new(device_node: PathBuf) -> MesaResult<LinuxPhysicalDevice> {
        let descriptor: OwnedDescriptor = OpenOptions::new()
            .read(true)
            .write(true)
            .open(device_node.clone())?
            .into();

        // TODO: confirm if necessary if everything has PCI-ID
        let name = get_drm_device_name(&descriptor)?;
        println!("the name is {}", name);

        Ok(LinuxPhysicalDevice { descriptor, name })
    }
}

impl PlatformPhysicalDevice for LinuxPhysicalDevice {
    fn as_fd(&self) -> Option<BorrowedFd<'_>> {
        Some(self.descriptor.as_fd())
    }

    fn as_raw_descriptor(&self) -> RawDescriptor {
        self.descriptor.as_raw_descriptor()
    }

    fn cpu_map(&self, offset: u64, size: usize) -> MesaResult<MemoryMapping> {
        MemoryMapping::from_offset(&self.descriptor, offset.try_into()?, size)
    }

    fn export(&self, gem_handle: u32) -> MesaResult<MesaHandle> {
        let mut arg: drm_prime_handle = drm_prime_handle {
            handle: gem_handle,
            flags: (O_CLOEXEC | O_RDWR) as u32,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_prime_handle
        let fd = unsafe {
            drm_ioctl_prime_handle_to_fd(self.descriptor.as_fd(), &mut arg)?;
            arg.fd
        };

        // SAFETY:
        // `fd` is valid after a successful PRIME_HANDLE_TO_HANDLE syscall.
        let descriptor = unsafe { OwnedDescriptor::from_raw_descriptor(fd) };

        Ok(MesaHandle {
            os_handle: descriptor,
            handle_type: MESA_HANDLE_TYPE_MEM_DMABUF,
        })
    }

    fn import(&self, handle: MesaHandle) -> MesaResult<u32> {
        let mut arg: drm_prime_handle = drm_prime_handle {
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_prime_handle
        let handle = unsafe {
            arg.fd = handle.os_handle.as_raw_descriptor();
            drm_ioctl_prime_fd_to_handle(self.descriptor.as_fd(), &mut arg)?;
            arg.handle
        };

        Ok(handle)
    }

    fn close(&self, gem_handle: u32) {
        let arg: drm_gem_close = drm_gem_close {
            handle: gem_handle,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_gem_handle
        let result = unsafe { drm_ioctl_gem_close(self.descriptor.as_fd(), &arg) };

        log_status!(result);
    }
}

impl AsVirtGpu for LinuxPhysicalDevice {}
impl PhysicalDevice for LinuxPhysicalDevice {}

// Helper function to parse hexadecimal string to u16
fn parse_hex_u16(s: &str) -> MesaResult<u16> {
    let valid_str = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    Ok(u16::from_str_radix(valid_str, 16)?)
}

pub fn enumerate_devices() -> MesaResult<Vec<MagmaPhysicalDevice>> {
    let mut devices: Vec<MagmaPhysicalDevice> = Vec::new();
    let dir_fd = open(
        DRM_DIR_NAME,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    let dir = Dir::new(dir_fd)?;
    for entry in dir.flatten() {
        let filename = entry.file_name().to_str()?;
        if filename.contains(DRM_RENDER_MINOR_NAME) {
            let path = Path::new(DRM_DIR_NAME).join(filename);
            let statbuf = stat(&path)?;

            let maj = major(statbuf.st_rdev);
            let min = minor(statbuf.st_rdev);

            let pci_device_dir = format!("/sys/dev/char/{}:{}/device", maj, min);
            let pci_subsystem_dir = format!("{}/subsystem", pci_device_dir);
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

            let mut pci_info: MagmaPciInfo = Default::default();
            let mut pci_bus_info: MagmaPciBusInfo = Default::default();
            for attr in PCI_ATTRS {
                let attr_path = format!("{}/{}", pci_device_dir, attr);
                let mut file = File::open(attr_path)?;
                let mut hex_string = String::new();
                file.read_to_string(&mut hex_string)?;

                match attr {
                    "revision" => pci_info.revision_id = parse_hex_u16(&hex_string)?.try_into()?,
                    "vendor" => pci_info.vendor_id = parse_hex_u16(&hex_string)?,
                    "device" => pci_info.device_id = parse_hex_u16(&hex_string)?,
                    "subsystem_vendor" => pci_info.subvendor_id = parse_hex_u16(&hex_string)?,
                    "subsystem_device" => pci_info.subdevice_id = parse_hex_u16(&hex_string)?,
                    _ => unimplemented!(),
                }
            }

            let uevent_path = format!("{}/uevent", pci_device_dir);
            let text: String = fs::read_to_string(uevent_path)?;
            for line in text.lines() {
                if line.contains("PCI_SLOT_NAME") {
                    let v: Vec<&str> = line.split(&['=', ':', '.'][..]).collect();

                    pci_bus_info.domain = v[1].parse::<u16>()?;
                    pci_bus_info.bus = v[2].parse::<u8>()?;
                    pci_bus_info.device = v[3].parse::<u8>()?;
                    pci_bus_info.function = v[4].parse::<u8>()?;
                }
            }

            devices.push(MagmaPhysicalDevice::new(
                Arc::new(LinuxPhysicalDevice::new(path.to_path_buf())?),
                pci_info,
                pci_bus_info,
            ));
        }
    }

    Ok(devices)
}

unsafe impl Send for LinuxPhysicalDevice {}
unsafe impl Sync for LinuxPhysicalDevice {}
