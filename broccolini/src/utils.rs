// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use magma_gpu::protocols::broccolini_protocol::broccolini_ctrl_hdr;
use magma_gpu::protocols::broccolini_protocol::BroccoliniProtocol;
use magma_gpu::protocols::broccolini_protocol::BROCCOLINI_RESP_NODATA;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::Error as MagmaGpuError;
use remain::sorted;
use rutabaga_gfx::RutabagaError;
use thiserror::Error;

#[sorted]
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum BroccoliniError {
    #[error("probe child returned no capsets")]
    EmptyCapsetsFromProbeChild,
    #[error("failed to create socketpair for worker: {0}")]
    FailedToCreateSocketpair(#[source] std::io::Error),
    #[error("failed to fork worker: {0}")]
    FailedToForkWorker(#[source] std::io::Error),
    #[error("failed to lock fence signalers")]
    FailedToLockFenceSignalers,
    #[error("no valid rutabaga capsets in --context-types '{0}'")]
    InvalidContextTypes(String),
    #[error("magma_gpu error: {0}")]
    MagmaGpuError(#[from] MagmaGpuError),
    #[error("missing context_id in CtxCreate header")]
    MissingContextId,
    #[error("rutabaga error: {0}")]
    RutabagaError(#[from] RutabagaError),
    #[allow(dead_code)]
    #[error("unimplemented")]
    Unimplemented,
}

pub type BroccoliniResult<T> = std::result::Result<T, BroccoliniError>;

pub fn send_ok_nodata(stream: &mut Stream<BroccoliniProtocol>) -> Result<(), MagmaGpuError> {
    stream.write(Message::Cmd(broccolini_ctrl_hdr {
        type_: BROCCOLINI_RESP_NODATA,
        payload: 0,
    }))
}
