// Copyright 2020 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! virgl_renderer: Handles 3D virtio-gpu hypercalls using virglrenderer.
//! External code found at <https://gitlab.freedesktop.org/virgl/virglrenderer/>.

#![cfg(feature = "virgl_renderer")]

use std::ffi::CStr;
use std::fs::canonicalize;
use std::fs::OpenOptions;
use std::io::Error as SysError;
use std::io::IoSlice;
use std::io::IoSliceMut;
use std::mem::size_of;
use std::mem::ManuallyDrop;
use std::os::fd::IntoRawFd;
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_void;
use std::os::unix::fs::OpenOptionsExt;
use std::panic::catch_unwind;
use std::process::abort;
use std::ptr::null_mut;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use log::error;
use log::info;
use log::log;
use log::warn;
use log::Level;
use mesa3d_util::FromRawDescriptor;
use mesa3d_util::IntoRawDescriptor;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaMapping;
use mesa3d_util::OwnedDescriptor;
use mesa3d_util::RawDescriptor;
use mesa3d_util::MESA_HANDLE_TYPE_MEM_DMABUF;
use mesa3d_util::MESA_HANDLE_TYPE_MEM_OPAQUE_FD;
use mesa3d_util::MESA_HANDLE_TYPE_MEM_SHM;

use crate::generated::virgl_renderer_bindings::*;
use crate::renderer_utils::ret_to_res;
use crate::renderer_utils::RutabagaCookie;
use crate::renderer_utils::VirglBox;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_core::RutabagaResource;
use crate::rutabaga_utils::Resource3DInfo;
use crate::rutabaga_utils::ResourceCreate3D;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::Transfer3D;
use crate::rutabaga_utils::VirglRendererFlags;
use crate::rutabaga_utils::RUTABAGA_FLAG_FENCE;
use crate::rutabaga_utils::RUTABAGA_FLAG_INFO_RING_IDX;
use crate::rutabaga_utils::RUTABAGA_MAP_ACCESS_RW;
use crate::RutabagaPath;
use crate::RutabagaPaths;
use crate::RUTABAGA_PATH_TYPE_GPU;

type Query = virgl_renderer_export_query;

/// Default drm fd, returning this indicates that virglrenderer should
/// find an available GPU itself.
const DEFAULT_DRM_FD: i32 = -1;

/// Check if the given rutabaga path is a valid GPU path.
fn is_valid_gpu_path(rpath: &RutabagaPath) -> bool {
    if rpath.path_type != RUTABAGA_PATH_TYPE_GPU {
        return false;
    }

    canonicalize(&rpath.path)
        .map(|path| path.starts_with("/dev/dri/renderD") && path.exists())
        .unwrap_or_default()
}

fn dup(rd: RawDescriptor) -> RutabagaResult<OwnedDescriptor> {
    // SAFETY:
    // Safe because the underlying raw descriptor is guaranteed valid by rd's existence.
    //
    // Note that we are cloning the underlying raw descriptor since we have no guarantee of
    // its existence after this function returns.
    let rd_as_safe_desc = ManuallyDrop::new(unsafe { OwnedDescriptor::from_raw_descriptor(rd) });

    // We have to clone rd because we have no guarantee ownership was transferred (rd is
    // borrowed).
    Ok(rd_as_safe_desc.try_clone().map_err(MesaError::IoError)?)
}

/// The virtio-gpu backend state tracker which supports accelerated rendering.
pub struct VirglRenderer {}

struct VirglRendererContext {
    ctx_id: u32,
}

fn import_resource(resource: &mut RutabagaResource) -> RutabagaResult<()> {
    if (resource.component_mask & (1 << (RutabagaComponentType::VirglRenderer as u8))) != 0 {
        return Ok(());
    }

    if let Some(handle) = &resource.handle {
        if handle.handle_type == MESA_HANDLE_TYPE_MEM_DMABUF {
            let dmabuf_fd = handle
                .os_handle
                .try_clone()
                .map_err(MesaError::IoError)?
                .into_raw_descriptor();
            // SAFETY:
            // Safe because we are being passed a valid fd
            unsafe {
                let dmabuf_size = libc::lseek64(dmabuf_fd, 0, libc::SEEK_END);
                libc::lseek64(dmabuf_fd, 0, libc::SEEK_SET);
                let args = virgl_renderer_resource_import_blob_args {
                    res_handle: resource.resource_id,
                    blob_mem: resource.blob_mem,
                    fd_type: VIRGL_RENDERER_BLOB_FD_TYPE_DMABUF,
                    fd: dmabuf_fd,
                    size: dmabuf_size as u64,
                };
                let ret = virgl_renderer_resource_import_blob(&args);
                if ret != 0 {
                    // import_blob can fail if we've previously imported this resource,
                    // but in any case virglrenderer does not take ownership of the fd
                    // in error paths
                    //
                    // Because of the re-import case we must still fall through to the
                    // virgl_renderer_ctx_attach_resource() call.
                    libc::close(dmabuf_fd);
                    return Ok(());
                }
                resource.component_mask |= 1 << (RutabagaComponentType::VirglRenderer as u8);
            }
        }
    }

    Ok(())
}

impl RutabagaContext for VirglRendererContext {
    fn submit_cmd(
        &mut self,
        commands: &mut [u8],
        fence_ids: &[u64],
        _shareable_fences: Vec<MesaHandle>,
    ) -> RutabagaResult<()> {
        #[cfg(not(virgl_renderer_unstable))]
        if !fence_ids.is_empty() {
            return Err(MesaError::Unsupported.into());
        }
        if commands.len() % size_of::<u32>() != 0 {
            return Err(RutabagaError::InvalidCommandSize(commands.len()));
        }
        let dword_count = (commands.len() / size_of::<u32>()) as i32;
        #[cfg(not(virgl_renderer_unstable))]
        // SAFETY:
        // Safe because the context and buffer are valid and virglrenderer will have been
        // initialized if there are Context instances.
        let ret = unsafe {
            virgl_renderer_submit_cmd(
                commands.as_mut_ptr() as *mut c_void,
                self.ctx_id as i32,
                dword_count,
            )
        };
        #[cfg(virgl_renderer_unstable)]
        // SAFETY:
        // Safe because the context and buffers are valid and virglrenderer will have been
        // initialized if there are Context instances.
        let ret = unsafe {
            virgl_renderer_submit_cmd2(
                commands.as_mut_ptr() as *mut c_void,
                self.ctx_id as i32,
                dword_count,
                fence_ids.as_ptr() as *mut u64,
                fence_ids.len() as u32,
            )
        };
        ret_to_res(ret)
    }

    fn attach(&mut self, resource: &mut RutabagaResource) {
        match import_resource(resource) {
            Ok(()) => (),
            Err(e) => error!("importing resource failing with {}", e),
        }

        // SAFETY:
        // The context id and resource id must be valid because the respective instances ensure
        // their lifetime.
        unsafe {
            virgl_renderer_ctx_attach_resource(self.ctx_id as i32, resource.resource_id as i32);
        }
    }

    fn detach(&mut self, resource: &RutabagaResource) {
        // SAFETY:
        // The context id and resource id must be valid because the respective instances ensure
        // their lifetime.
        unsafe {
            virgl_renderer_ctx_detach_resource(self.ctx_id as i32, resource.resource_id as i32);
        }
    }

    fn component_type(&self) -> RutabagaComponentType {
        RutabagaComponentType::VirglRenderer
    }

    fn context_create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<Option<MesaHandle>> {
        // RutabagaFence::flags are not compatible with virglrenderer's fencing API and currently
        // virglrenderer context's assume all fences on a single timeline are MERGEABLE, and enforce
        // this assumption.
        let flags: u32 = VIRGL_RENDERER_FENCE_FLAG_MERGEABLE;

        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let ret = unsafe {
            virgl_renderer_context_create_fence(
                fence.ctx_id,
                flags,
                fence.ring_idx as u32,
                fence.fence_id,
            )
        };
        ret_to_res(ret)?;
        Ok(None)
    }
}

impl Drop for VirglRendererContext {
    fn drop(&mut self) {
        // SAFETY:
        // The context is safe to destroy because nothing else can be referencing it.
        unsafe {
            virgl_renderer_context_destroy(self.ctx_id);
        }
    }
}

extern "C" fn log_callback(
    log_level: virgl_log_level_flags,
    message: *const ::std::os::raw::c_char,
    _user_data: *mut ::std::os::raw::c_void,
) {
    let level = match log_level {
        VIRGL_LOG_LEVEL_DEBUG => Level::Debug,
        VIRGL_LOG_LEVEL_WARNING => Level::Warn,
        VIRGL_LOG_LEVEL_ERROR => Level::Error,
        VIRGL_LOG_LEVEL_INFO => Level::Info,
        _ => Level::Trace,
    };

    // SAFETY:
    // The caller ensures that `message` is always a valid pointer to a NULL-terminated string
    // (even if zero-length).
    let message_str = unsafe { CStr::from_ptr(message) };
    log!(level, "{}", message_str.to_string_lossy());
}

extern "C" fn get_drm_fd(cookie: *mut c_void) -> c_int {
    catch_unwind(|| {
        assert!(!cookie.is_null());
        // SAFETY:
        // The assert above ensures it's not null, and virglrenderer ensures the pointer
        // is valid for the duration of this callback.
        let cookie = unsafe { &mut *(cookie as *mut RutabagaCookie) };

        // Find the first valid GPU path from rutabaga paths
        let gpu_path = cookie.rutabaga_paths.as_ref().and_then(|rpaths| {
            rpaths
                .iter()
                .find(|rpath| is_valid_gpu_path(rpath))
                .map(|rpath| rpath.path.clone())
        });

        // Try to open the path and return its fd
        gpu_path
            .and_then(|path| {
                info!("virglrenderer: using GPU path {path:?}");
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK | libc::O_NOCTTY)
                    .open(path)
                    .inspect_err(|err| error!("failed to open GPU path: {err}"))
                    .ok()
            })
            // Convert file to raw fd, the ownership of the fd is
            // transferred to virglrenderer.
            .map(|file| file.into_raw_fd())
            // If no path was provided or opening it failed, use the
            // default drm fd.
            .unwrap_or(DEFAULT_DRM_FD)
    })
    .unwrap_or_else(|_| abort())
}

extern "C" fn write_context_fence(cookie: *mut c_void, ctx_id: u32, ring_idx: u32, fence_id: u64) {
    catch_unwind(|| {
        assert!(!cookie.is_null());
        // SAFETY:
        // The assert above ensures it's not null, and virglrenderer ensures the pointer
        // is valid for the duration of this callback.
        let cookie = unsafe { &*(cookie as *mut RutabagaCookie) };

        // Call fence completion callback
        if let Some(handler) = &cookie.fence_handler {
            handler.call(RutabagaFence {
                flags: RUTABAGA_FLAG_FENCE | RUTABAGA_FLAG_INFO_RING_IDX,
                fence_id,
                ctx_id,
                ring_idx: ring_idx as u8,
            });
        }
    })
    .unwrap_or_else(|_| abort())
}

extern "C" fn write_fence(cookie: *mut c_void, fence: u32) {
    catch_unwind(|| {
        assert!(!cookie.is_null());
        // SAFETY:
        // The assert above ensures it's not null, and virglrenderer ensures the pointer
        // is valid for the duration of this callback.
        let cookie = unsafe { &*(cookie as *mut RutabagaCookie) };

        // Call fence completion callback
        if let Some(handler) = &cookie.fence_handler {
            handler.call(RutabagaFence {
                flags: RUTABAGA_FLAG_FENCE,
                fence_id: fence as u64,
                ctx_id: 0,
                ring_idx: 0,
            });
        }
    })
    .unwrap_or_else(|_| abort())
}

extern "C" fn get_server_fd(cookie: *mut c_void, version: u32) -> c_int {
    catch_unwind(|| {
        assert!(!cookie.is_null());
        // SAFETY:
        // The assert above ensures it's not null, and virglrenderer ensures the pointer
        // is valid for the duration of this callback.
        let cookie = unsafe { &mut *(cookie as *mut RutabagaCookie) };

        if version != 0 {
            return -1;
        }

        // Transfer the fd ownership to virglrenderer.
        cookie
            .render_server_fd
            .take()
            .map(OwnedDescriptor::into_raw_descriptor)
            .unwrap_or(-1)
    })
    .unwrap_or_else(|_| abort())
}

const VIRGL_RENDERER_CALLBACKS: &virgl_renderer_callbacks = &virgl_renderer_callbacks {
    version: 3,
    write_fence: Some(write_fence),
    create_gl_context: None,
    destroy_gl_context: None,
    make_current: None,
    get_drm_fd: Some(get_drm_fd),
    write_context_fence: Some(write_context_fence),
    get_server_fd: Some(get_server_fd),
    get_egl_display: None,
};

/// Retrieves metadata suitable for export about this resource. If "export_fd" is true,
/// performs an export of this resource so that it may be imported by other processes.
fn export_query(resource_id: u32) -> RutabagaResult<Query> {
    let mut query: Query = Default::default();
    query.hdr.stype = VIRGL_RENDERER_STRUCTURE_TYPE_EXPORT_QUERY;
    query.hdr.stype_version = 0;
    query.hdr.size = size_of::<Query>() as u32;
    query.in_resource_id = resource_id;
    query.in_export_fds = 0;

    let ret =
        // SAFETY:
        // Safe because the image parameters are stack variables of the correct type.
        unsafe { virgl_renderer_execute(&mut query as *mut _ as *mut c_void, query.hdr.size) };

    ret_to_res(ret)?;
    Ok(query)
}

impl VirglRenderer {
    pub fn init(
        virglrenderer_flags: VirglRendererFlags,
        fence_handler: RutabagaFenceHandler,
        render_server_fd: Option<OwnedDescriptor>,
        rutabaga_paths: Option<RutabagaPaths>,
    ) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        if cfg!(debug_assertions) {
            // TODO(b/315870313): Add safety comment
            #[allow(clippy::undocumented_unsafe_blocks)]
            let ret = unsafe { libc::dup2(libc::STDOUT_FILENO, libc::STDERR_FILENO) };
            if ret == -1 {
                warn!(
                    "unable to dup2 stdout to stderr: {}",
                    SysError::last_os_error()
                );
            }
        }

        // virglrenderer is a global state backed library that uses thread bound OpenGL contexts.
        // Initialize it only once and use the non-send/non-sync Renderer struct to keep things tied
        // to whichever thread called this function first.
        static INIT_ONCE: AtomicBool = AtomicBool::new(false);
        if INIT_ONCE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .is_err()
        {
            return Err(RutabagaError::AlreadyInUse);
        }

        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        unsafe {
            virgl_set_log_callback(Some(log_callback), null_mut(), None);
        };

        // Cookie is intentionally never freed because virglrenderer never gets uninitialized.
        // Otherwise, Resource and Context would become invalid because their lifetime is not tied
        // to the Renderer instance. Doing so greatly simplifies the ownership for users of this
        // library.
        let cookie = Box::into_raw(Box::new(RutabagaCookie {
            render_server_fd,
            fence_handler: Some(fence_handler),
            debug_handler: None,
            rutabaga_paths,
        }));

        // SAFETY:
        // Safe because a valid cookie and set of callbacks is used and the result is checked for
        // error.
        let ret = unsafe {
            virgl_renderer_init(
                cookie as *mut c_void,
                virglrenderer_flags.into(),
                VIRGL_RENDERER_CALLBACKS as *const virgl_renderer_callbacks
                    as *mut virgl_renderer_callbacks,
            )
        };

        ret_to_res(ret)?;
        Ok(Box::new(VirglRenderer {}))
    }

    fn map_info(&self, resource_id: u32) -> RutabagaResult<u32> {
        let mut map_info = 0;
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let ret = unsafe { virgl_renderer_resource_get_map_info(resource_id, &mut map_info) };
        ret_to_res(ret)?;

        Ok(map_info | RUTABAGA_MAP_ACCESS_RW)
    }

    fn query(&self, resource_id: u32) -> RutabagaResult<Resource3DInfo> {
        let query = export_query(resource_id)?;
        if query.out_num_fds == 0 {
            return Err(MesaError::Unsupported.into());
        }

        // virglrenderer unfortunately doesn't return the width or height, so map to zero.
        Ok(Resource3DInfo {
            width: 0,
            height: 0,
            drm_fourcc: query.out_fourcc,
            strides: query.out_strides,
            offsets: query.out_offsets,
            modifier: query.out_modifier,
        })
    }

    fn export_blob(&self, resource_id: u32) -> RutabagaResult<Arc<MesaHandle>> {
        let mut fd_type = 0;
        let mut fd = 0;
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let ret =
            unsafe { virgl_renderer_resource_export_blob(resource_id, &mut fd_type, &mut fd) };
        ret_to_res(ret)?;

        // SAFETY:
        // Safe because the FD was just returned by a successful virglrenderer
        // call so it must be valid and owned by us.
        let handle = unsafe { OwnedDescriptor::from_raw_descriptor(fd) };

        let handle_type = match fd_type {
            VIRGL_RENDERER_BLOB_FD_TYPE_DMABUF => MESA_HANDLE_TYPE_MEM_DMABUF,
            VIRGL_RENDERER_BLOB_FD_TYPE_SHM => MESA_HANDLE_TYPE_MEM_SHM,
            VIRGL_RENDERER_BLOB_FD_TYPE_OPAQUE => MESA_HANDLE_TYPE_MEM_OPAQUE_FD,
            _ => {
                return Err(MesaError::Unsupported.into());
            }
        };

        Ok(Arc::new(MesaHandle {
            os_handle: handle,
            handle_type,
        }))
    }
}

impl Drop for VirglRenderer {
    fn drop(&mut self) {
        // SAFETY:
        // Safe because virglrenderer is initialized.
        //
        // This invalidates all context ids and resource ids.  It is fine because struct Rutabaga
        // makes sure contexts and resources are dropped before this is reached.  Even if it did
        // not, virglrenderer is designed to deal with invalid ids safely.
        unsafe {
            virgl_renderer_cleanup(null_mut());
        }
    }
}

impl RutabagaComponent for VirglRenderer {
    fn get_capset_info(&self, capset_id: u32) -> (u32, u32) {
        let mut version = 0;
        let mut size = 0;
        // SAFETY:
        // Safe because virglrenderer is initialized by now and properly size stack variables are
        // used for the pointers.
        unsafe {
            virgl_renderer_get_cap_set(capset_id, &mut version, &mut size);
        }
        (version, size)
    }

    fn get_capset(&self, capset_id: u32, version: u32) -> Vec<u8> {
        let (_, max_size) = self.get_capset_info(capset_id);
        let mut buf = vec![0u8; max_size as usize];
        // SAFETY:
        // Safe because virglrenderer is initialized by now and the given buffer is sized properly
        // for the given cap id/version.
        unsafe {
            virgl_renderer_fill_caps(capset_id, version, buf.as_mut_ptr() as *mut c_void);
        }
        buf
    }

    fn force_ctx_0(&self) {
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        unsafe {
            virgl_renderer_force_ctx_0()
        };
    }

    fn create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<()> {
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let ret = unsafe { virgl_renderer_create_fence(fence.fence_id as i32, fence.ctx_id) };
        ret_to_res(ret)
    }

    fn event_poll(&self) {
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        unsafe {
            virgl_renderer_poll()
        };
    }

    fn poll_descriptor(&self) -> Option<OwnedDescriptor> {
        // SAFETY:
        // Safe because it can be called anytime and returns -1 in the event of an error.
        let fd = unsafe { virgl_renderer_get_poll_fd() };
        if fd >= 0 {
            let descriptor: RawDescriptor = fd as RawDescriptor;
            if let Ok(dup_fd) = dup(descriptor) {
                return Some(dup_fd);
            }
        }
        None
    }

    fn create_3d(
        &self,
        resource_id: u32,
        resource_create_3d: ResourceCreate3D,
    ) -> RutabagaResult<RutabagaResource> {
        let mut args = virgl_renderer_resource_create_args {
            handle: resource_id,
            target: resource_create_3d.target,
            format: resource_create_3d.format,
            bind: resource_create_3d.bind,
            width: resource_create_3d.width,
            height: resource_create_3d.height,
            depth: resource_create_3d.depth,
            array_size: resource_create_3d.array_size,
            last_level: resource_create_3d.last_level,
            nr_samples: resource_create_3d.nr_samples,
            flags: resource_create_3d.flags,
        };

        // SAFETY:
        // Safe because virglrenderer is initialized by now, and the return value is checked before
        // returning a new resource. The backing buffers are not supplied with this call.
        let ret = unsafe { virgl_renderer_resource_create(&mut args, null_mut(), 0) };
        ret_to_res(ret)?;

        let mut resource_handle: Option<Arc<MesaHandle>> = self.export_blob(resource_id).ok();
        let mut resource_info_3d: Option<Resource3DInfo> = self.query(resource_id).ok();

        // Fallback if export_blob and query both fail to return a DMABUF handle or 3D info.
        if resource_handle.is_none() && resource_info_3d.is_none() {
            let mut info_ext = Default::default();

            // SAFETY: virglrenderer is initialized; info_ext is a valid pointer.
            // Function writes into info_ext but does not retain the pointer after returning.
            let ret_info =
                unsafe { virgl_renderer_resource_get_info_ext(resource_id as i32, &mut info_ext) };

            if ret_info == 0 {
                // Successfully got info_ext, now try to get the FD.
                let mut fd = -1;

                // SAFETY: virglrenderer is initialized; tex_id is from valid resource.
                // fd is written by the call and remains owned by the caller.
                let ret_fd =
                    unsafe { virgl_renderer_get_fd_for_texture(info_ext.base.tex_id, &mut fd) };

                if ret_fd == 0 && fd >= 0 {
                    // Successfully got DMABUF FD.
                    let fourcc: u32 = info_ext.base.drm_fourcc as u32;

                    // SAFETY: `fd` is validated to be >= 0 and uniquely owned.
                    let owned_fd = unsafe { OwnedDescriptor::from_raw_descriptor(fd) };

                    resource_handle = Some(Arc::new(MesaHandle {
                        os_handle: owned_fd,
                        handle_type: MESA_HANDLE_TYPE_MEM_DMABUF,
                    }));
                    resource_info_3d = Some(Resource3DInfo {
                        width: info_ext.base.width,
                        height: info_ext.base.height,
                        drm_fourcc: fourcc,
                        strides: [info_ext.base.stride, 0, 0, 0], // Assuming single plane
                        offsets: [0, 0, 0, 0],                    // Assuming single plane
                        modifier: info_ext.modifiers,
                    });
                }
            }
        }

        Ok(RutabagaResource {
            resource_id,
            handle: resource_handle,
            blob: false,
            blob_mem: 0,
            blob_flags: 0,
            map_info: None,
            info_2d: None,
            info_3d: resource_info_3d,
            vulkan_info: None,
            backing_iovecs: None,
            component_mask: 1 << (RutabagaComponentType::VirglRenderer as u8),
            size: 0,
            mapping: None,
            guest_cpu_mappable: false,
        })
    }

    fn attach_backing(
        &self,
        resource_id: u32,
        vecs: &mut Vec<RutabagaIovec>,
    ) -> RutabagaResult<()> {
        // SAFETY:
        // Safe because the backing is into guest memory that we store a reference count for.
        let ret = unsafe {
            virgl_renderer_resource_attach_iov(
                resource_id as i32,
                vecs.as_mut_ptr() as *mut iovec,
                vecs.len() as i32,
            )
        };
        ret_to_res(ret)
    }

    fn detach_backing(&self, resource_id: u32) {
        // SAFETY:
        // Safe as we don't need the old backing iovecs returned and the reference to the guest
        // memory can be dropped as it will no longer be needed for this resource.
        unsafe {
            virgl_renderer_resource_detach_iov(resource_id as i32, null_mut(), null_mut());
        }
    }

    fn unref_resource(&self, resource_id: u32) {
        // SAFETY:
        // The resource is safe to unreference destroy because no user of these bindings can still
        // be holding a reference.
        unsafe {
            virgl_renderer_resource_unref(resource_id);
        }
    }

    fn transfer_write(
        &self,
        ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSlice>,
    ) -> RutabagaResult<()> {
        if transfer.is_empty() {
            return Ok(());
        }

        if buf.is_some() {
            return Err(MesaError::Unsupported.into());
        }

        let mut transfer_box = VirglBox {
            x: transfer.x,
            y: transfer.y,
            z: transfer.z,
            w: transfer.w,
            h: transfer.h,
            d: transfer.d,
        };

        // SAFETY:
        // Safe because only stack variables of the appropriate type are used.
        let ret = unsafe {
            virgl_renderer_transfer_write_iov(
                resource.resource_id,
                ctx_id,
                transfer.level as i32,
                transfer.stride,
                transfer.layer_stride,
                &mut transfer_box as *mut VirglBox as *mut virgl_box,
                transfer.offset,
                null_mut(),
                0,
            )
        };
        ret_to_res(ret)
    }

    fn transfer_read(
        &self,
        ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSliceMut>,
    ) -> RutabagaResult<()> {
        if transfer.is_empty() {
            return Ok(());
        }

        let mut transfer_box = VirglBox {
            x: transfer.x,
            y: transfer.y,
            z: transfer.z,
            w: transfer.w,
            h: transfer.h,
            d: transfer.d,
        };

        let mut iov = RutabagaIovec {
            base: null_mut(),
            len: 0,
        };

        let (iovecs, num_iovecs) = match buf {
            Some(mut buf) => {
                iov.base = buf.as_mut_ptr() as *mut c_void;
                iov.len = buf.len();
                (&mut iov as *mut RutabagaIovec as *mut iovec, 1)
            }
            None => (null_mut(), 0),
        };

        // SAFETY:
        // Safe because only stack variables of the appropriate type are used.
        let ret = unsafe {
            virgl_renderer_transfer_read_iov(
                resource.resource_id,
                ctx_id,
                transfer.level,
                transfer.stride,
                transfer.layer_stride,
                &mut transfer_box as *mut VirglBox as *mut virgl_box,
                transfer.offset,
                iovecs,
                num_iovecs,
            )
        };
        ret_to_res(ret)
    }

    #[allow(unused_variables)]
    fn create_blob(
        &mut self,
        ctx_id: u32,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        mut iovec_opt: Option<Vec<RutabagaIovec>>,
        _handle_opt: Option<MesaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        let mut iovec_ptr = null_mut();
        let mut num_iovecs = 0;
        if let Some(ref mut iovecs) = iovec_opt {
            iovec_ptr = iovecs.as_mut_ptr();
            num_iovecs = iovecs.len();
        }

        let resource_create_args = virgl_renderer_resource_create_blob_args {
            res_handle: resource_id,
            ctx_id,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            blob_id: resource_create_blob.blob_id,
            size: resource_create_blob.size,
            iovecs: iovec_ptr as *const iovec,
            num_iovs: num_iovecs as u32,
        };

        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let ret = unsafe { virgl_renderer_resource_create_blob(&resource_create_args) };
        ret_to_res(ret)?;

        // TODO(b/244591751): assign vulkan_info to support opaque_fd mapping via Vulkano when
        // sandboxing (hence external_blob) is enabled.
        Ok(RutabagaResource {
            resource_id,
            handle: self.export_blob(resource_id).ok(),
            blob: true,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            map_info: self.map_info(resource_id).ok(),
            info_2d: None,
            info_3d: self.query(resource_id).ok(),
            vulkan_info: None,
            backing_iovecs: iovec_opt,
            component_mask: 1 << (RutabagaComponentType::VirglRenderer as u8),
            size: resource_create_blob.size,
            mapping: None,
            guest_cpu_mappable: false,
        })
    }

    fn map(&self, resource_id: u32) -> RutabagaResult<MesaMapping> {
        let mut map: *mut c_void = null_mut();
        let mut size: u64 = 0;
        // SAFETY:
        // Safe because virglrenderer wraps and validates use of GL/VK.
        let ret = unsafe { virgl_renderer_resource_map(resource_id, &mut map, &mut size) };
        if ret != 0 {
            return Err(RutabagaError::MappingFailed(ret));
        }

        Ok(MesaMapping {
            ptr: map as u64,
            size,
        })
    }

    fn unmap(&self, resource_id: u32) -> RutabagaResult<()> {
        // SAFETY:
        // Safe because virglrenderer is initialized by now.
        let ret = unsafe { virgl_renderer_resource_unmap(resource_id) };
        ret_to_res(ret)
    }

    #[allow(unused_variables)]
    fn export_fence(&self, fence_id: u64) -> RutabagaResult<MesaHandle> {
        #[cfg(virgl_renderer_unstable)]
        {
            let mut fd: i32 = 0;
            // SAFETY:
            // Safe because the parameters are stack variables of the correct type.
            let ret = unsafe { virgl_renderer_export_fence(fence_id, &mut fd) };
            ret_to_res(ret)?;

            // SAFETY:
            // Safe because the FD was just returned by a successful virglrenderer call so it must
            // be valid and owned by us.
            let fence = unsafe { OwnedDescriptor::from_raw_descriptor(fd) };
            Ok(MesaHandle {
                os_handle: fence,
                handle_type: MESA_HANDLE_TYPE_SIGNAL_SYNC_FD,
            })
        }
        #[cfg(not(virgl_renderer_unstable))]
        Err(MesaError::Unsupported.into())
    }

    #[allow(unused_variables)]
    fn create_context(
        &self,
        ctx_id: u32,
        context_init: u32,
        context_name: Option<&str>,
        _fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaContext>> {
        let mut name: &str = "gpu_renderer";
        if let Some(name_string) = context_name.filter(|s| !s.is_empty()) {
            name = name_string;
        }

        // SAFETY:
        // Safe because virglrenderer is initialized by now and the context name is statically
        // allocated. The return value is checked before returning a new context.
        let ret = unsafe {
            match context_init {
                0 => virgl_renderer_context_create(
                    ctx_id,
                    name.len() as u32,
                    name.as_ptr() as *const c_char,
                ),
                _ => virgl_renderer_context_create_with_flags(
                    ctx_id,
                    context_init,
                    name.len() as u32,
                    name.as_ptr() as *const c_char,
                ),
            }
        };
        ret_to_res(ret)?;
        Ok(Box::new(VirglRendererContext { ctx_id }))
    }
}
