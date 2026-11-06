// Copyright 2024 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

mod kumquat;
mod kumquat_gpu;

use clap::Parser;
use kumquat::KumquatBuilder;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::IntoRawDescriptor;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::WritePipe;

use crate::kumquat_gpu::KumquatGpuResult;

#[derive(Parser, Debug)]
#[command(version = "1.71", about = None, long_about = None)]
struct Args {
    /// Colon-separated list of virtio-gpu capsets.  For example,
    /// "--capset-names=gfxstream-vulkan:cross-domain"
    #[arg(long, default_value = "gfxstream-vulkan")]
    capset_names: String,

    /// Colon-separated list of virtio-gpu capsets routed to the broccolini render server.
    /// For example, "--remote-capset-names=gfxstream-vulkan:cross-domain"
    #[arg(long, alias = "remote_capset_names", default_value = "")]
    remote_capset_names: String,

    /// Path to the built broccolini render server binary.
    #[arg(long, alias = "broccolini_path", default_value = "")]
    broccolini_path: String,

    /// Path to the emulated virtio-gpu socket.
    #[arg(long, default_value = "/tmp/kumquat-gpu-0")]
    gpu_socket_path: String,

    /// Opaque renderer specific features
    #[arg(long, default_value = "")]
    renderer_features: String,

    /// An OS-specific pipe descriptor to the parent process
    #[arg(long, default_value = "0")]
    pipe_descriptor: i64,
}

fn main() -> KumquatGpuResult<()> {
    let args = Args::parse();

    let mut kumquat = KumquatBuilder::new()
        .set_capset_names(args.capset_names)
        .set_remote_capset_names(args.remote_capset_names)
        .set_broccolini_path((!args.broccolini_path.is_empty()).then_some(args.broccolini_path))
        .set_gpu_socket((!args.gpu_socket_path.is_empty()).then_some(args.gpu_socket_path))
        .set_renderer_features(args.renderer_features)
        .build()?;

    if args.pipe_descriptor != 0 {
        // SAFETY: the caller passed a descriptor it opened and handed over.
        let descriptor = unsafe {
            OwnedDescriptor::from_raw_descriptor(args.pipe_descriptor.into_raw_descriptor())
        };
        let write_pipe = WritePipe::new(descriptor);
        write_pipe.write(&1u64.to_le_bytes())?;
    }

    loop {
        kumquat.run()?;
    }
}
