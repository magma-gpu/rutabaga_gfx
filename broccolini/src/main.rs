// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

//! Single-threaded Zygote render server (`broccolini`) for Rutabaga multi-process context isolation.

use clap::Parser;
use magma_gpu::util::RawDescriptor;
use remain::sorted;
use rutabaga_gfx::calculate_capset_mask;
use thiserror::Error;

#[sorted]
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum BroccoliniError {
    #[error("no valid rutabaga capsets in --context-types '{0}'")]
    InvalidContextTypes(String),
}

pub type BroccoliniResult<T> = std::result::Result<T, BroccoliniError>;

#[derive(Parser, Debug)]
#[command(version = "0.1.7", about = None, long_about = None)]
#[allow(dead_code)]
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

    Ok(())
}
