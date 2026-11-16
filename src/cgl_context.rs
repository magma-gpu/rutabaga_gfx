//! OpenGL contexts for virglrenderer's vrend backend on macOS, backed by CGL.
//!
//! virglrenderer cannot create GL contexts on macOS by itself (there is no EGL), so the VMM
//! provides them through `create_gl_context` / `destroy_gl_context` / `make_current`.
//! Rendering goes to FBOs, so the contexts need no drawable.

use std::ffi::c_void;
use std::os::raw::c_int;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use log::error;

use crate::generated::virgl_renderer_bindings::{
    virgl_renderer_gl_context, virgl_renderer_gl_ctx_param,
};

type CGLContextObj = *mut c_void;
type CGLPixelFormatObj = *mut c_void;
type CGLError = c_int;

const K_CGL_PFA_ACCELERATED: c_int = 73;
const K_CGL_PFA_ALLOW_OFFLINE_RENDERERS: c_int = 96;
const K_CGL_PFA_OPENGL_PROFILE: c_int = 99;
const K_CGL_OGLP_VERSION_GL4_CORE: c_int = 0x4100;

const ACCELERATED_GL4_CORE: [c_int; 5] = [
    K_CGL_PFA_OPENGL_PROFILE,
    K_CGL_OGLP_VERSION_GL4_CORE,
    K_CGL_PFA_ACCELERATED,
    K_CGL_PFA_ALLOW_OFFLINE_RENDERERS,
    0,
];

#[link(name = "OpenGL", kind = "framework")]
extern "C" {
    fn CGLChoosePixelFormat(
        attribs: *const c_int,
        pix: *mut CGLPixelFormatObj,
        npix: *mut c_int,
    ) -> CGLError;
    fn CGLDestroyPixelFormat(pix: CGLPixelFormatObj) -> CGLError;
    fn CGLCreateContext(
        pix: CGLPixelFormatObj,
        share: CGLContextObj,
        ctx: *mut CGLContextObj,
    ) -> CGLError;
    fn CGLDestroyContext(ctx: CGLContextObj) -> CGLError;
    fn CGLSetCurrentContext(ctx: CGLContextObj) -> CGLError;
    fn CGLGetCurrentContext() -> CGLContextObj;
}

/// vrend's first, non-shared context. CGL contexts are current per thread, and some virglrenderer
/// entry points (e.g. virgl_renderer_fill_caps) query GL state without making a context current.
static MAIN_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Makes vrend's main context current on this thread if none is, so GL queries made outside vrend's
/// own context switches see the real host GL instead of no context at all.
pub fn ensure_current() {
    let main = MAIN_CONTEXT.load(Ordering::Acquire);
    // SAFETY: main is a live context created by create_gl_context, or null.
    unsafe {
        if !main.is_null() && CGLGetCurrentContext().is_null() {
            CGLSetCurrentContext(main);
        }
    }
}

/// Creates a GL 4.1 core context. A shared context shares objects with the current one, which is
/// how vrend sets up its per-guest-context contexts.
pub unsafe extern "C" fn create_gl_context(
    _cookie: *mut c_void,
    _scanout_idx: c_int,
    param: *mut virgl_renderer_gl_ctx_param,
) -> virgl_renderer_gl_context {
    let shared = !param.is_null() && (*param).shared;
    create_context(&ACCELERATED_GL4_CORE, shared)
}

unsafe fn create_context(attribs: &[c_int], shared: bool) -> CGLContextObj {
    let mut pix: CGLPixelFormatObj = ptr::null_mut();
    let mut npix: c_int = 0;
    if CGLChoosePixelFormat(attribs.as_ptr(), &mut pix, &mut npix) != 0 || pix.is_null() {
        error!("cgl: no matching GL 4.1 core pixel format");
        return ptr::null_mut();
    }
    let share = if shared {
        CGLGetCurrentContext()
    } else {
        ptr::null_mut()
    };
    let mut ctx: CGLContextObj = ptr::null_mut();
    let err = CGLCreateContext(pix, share, &mut ctx);
    CGLDestroyPixelFormat(pix);
    if err != 0 {
        error!("cgl: CGLCreateContext failed: {err}");
        return ptr::null_mut();
    }
    if !shared {
        let _ = MAIN_CONTEXT.compare_exchange(
            ptr::null_mut(),
            ctx,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
    ctx
}

pub unsafe extern "C" fn destroy_gl_context(_cookie: *mut c_void, ctx: virgl_renderer_gl_context) {
    if ctx.is_null() {
        return;
    }
    if CGLGetCurrentContext() == ctx {
        CGLSetCurrentContext(ptr::null_mut());
    }
    let _ =
        MAIN_CONTEXT.compare_exchange(ctx, ptr::null_mut(), Ordering::AcqRel, Ordering::Acquire);
    CGLDestroyContext(ctx);
}

pub unsafe extern "C" fn make_current(
    _cookie: *mut c_void,
    _scanout_idx: c_int,
    ctx: virgl_renderer_gl_context,
) -> c_int {
    CGLSetCurrentContext(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    const K_CGL_PFA_RENDERER_ID: c_int = 70;
    const K_CGL_RENDERER_GENERIC_FLOAT_ID: c_int = 0x00020400;

    // CI runners have no accelerated renderer; Apple's software renderer exercises the same paths.
    const SOFTWARE_GL4_CORE: [c_int; 5] = [
        K_CGL_PFA_OPENGL_PROFILE,
        K_CGL_OGLP_VERSION_GL4_CORE,
        K_CGL_PFA_RENDERER_ID,
        K_CGL_RENDERER_GENERIC_FLOAT_ID,
        0,
    ];

    // One test: MAIN_CONTEXT and the current context are process/thread state.
    #[test]
    fn context_lifecycle() {
        // SAFETY: contexts are created and destroyed on this thread and not used afterwards.
        unsafe {
            let main = create_context(&SOFTWARE_GL4_CORE, false);
            assert!(!main.is_null());
            assert_eq!(MAIN_CONTEXT.load(Ordering::Acquire), main);

            assert_eq!(make_current(ptr::null_mut(), 0, main), 0);
            assert_eq!(CGLGetCurrentContext(), main);

            let shared = create_context(&SOFTWARE_GL4_CORE, true);
            assert!(!shared.is_null());
            assert_eq!(MAIN_CONTEXT.load(Ordering::Acquire), main);
            assert_eq!(make_current(ptr::null_mut(), 0, shared), 0);
            destroy_gl_context(ptr::null_mut(), shared);
            assert!(CGLGetCurrentContext().is_null());

            ensure_current();
            assert_eq!(CGLGetCurrentContext(), main);

            destroy_gl_context(ptr::null_mut(), main);
            assert!(CGLGetCurrentContext().is_null());
            assert!(MAIN_CONTEXT.load(Ordering::Acquire).is_null());

            ensure_current();
            assert!(CGLGetCurrentContext().is_null());
        }
    }
}
