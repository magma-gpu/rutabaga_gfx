// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

//! Single-threaded Zygote render server (`broccolini`) for Rutabaga multi-process context isolation.

mod context_worker;
mod sys;
mod utils;
mod zygote;

use clap::Parser;
use magma_gpu::protocols::broccolini_protocol::BroccoliniProtocol;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::RawDescriptor;
use magma_gpu::util::Tube;
use rutabaga_gfx::calculate_capset_mask;

use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;
use crate::zygote::probe_capsets_in_child;
use crate::zygote::run_render_server_zygote_loop;

#[derive(Parser, Debug)]
#[command(version = "0.1.7", about = None, long_about = None)]
struct Args {
    /// The inherited socket file descriptor for the "main"/"parent" render server.
    #[arg(long)]
    socket_fd: RawDescriptor,

    /// Colon-separated list of virtio-gpu context types (e.g. "gfxstream-vulkan").
    #[arg(long, default_value = "")]
    context_types: String,

    /// The comma-separated list of renderer features.
    #[arg(long, default_value = "")]
    renderer_features: String,
}

fn main() -> BroccoliniResult<()> {
    let args = Args::parse();

    if args.context_types.is_empty() {
        return Err(BroccoliniError::InvalidContextTypes(args.context_types));
    }

    let capset_mask = calculate_capset_mask(args.context_types.split(':'));
    if capset_mask == 0 {
        return Err(BroccoliniError::InvalidContextTypes(args.context_types));
    }

    let cached_capsets = probe_capsets_in_child(capset_mask, &args.renderer_features)?;

    let desc = unsafe { OwnedDescriptor::from_raw_descriptor(args.socket_fd) };
    let tube = Tube::try_from(desc)?;
    let broccolini_control_stream = Stream::<BroccoliniProtocol>::new(tube);
    run_render_server_zygote_loop(
        broccolini_control_stream,
        &cached_capsets,
        capset_mask,
        &args.renderer_features,
    )?;

    Ok(())
}
