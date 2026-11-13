#!/usr/bin/env bash
# Copyright (C) 2026 The Magma GPU Project
# SPDX-License-Identifier: MIT

set -euo pipefail

LOCAL_GFXSTREAM_PATH=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --local-gfxstream-path=*)
            LOCAL_GFXSTREAM_PATH="${1#*=}"
            shift
            ;;
        --local-gfxstream-path)
            if [ "$#" -lt 2 ]; then
                echo "Error: --local-gfxstream-path requires a path argument." >&2
                exit 1
            fi
            LOCAL_GFXSTREAM_PATH="$2"
            shift 2
            ;;
        --)
            shift
            break
            ;;
        -*)
            echo "Unknown option: $1" >&2
            echo "Usage: $0 [--local-gfxstream-path=<path>] [--] <command> [args...]" >&2
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

if [ "$#" -eq 0 ]; then
    echo "Usage: $0 [--local-gfxstream-path=<path>] [--] <command> [args...]" >&2
    echo "Example: $0 --local-gfxstream-path=/path/to/gfxstream -- vkcube" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUTABAGA_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${RUTABAGA_DIR}/build"

GUEST_GFXSTREAM_DIR="${BUILD_DIR}/guest_gfxstream"
GUEST_GFXSTREAM_BUILD_DIR="${GUEST_GFXSTREAM_DIR}/guest-build"

if [ -n "${LOCAL_GFXSTREAM_PATH}" ]; then
    HOST_GFXSTREAM_DIR="$(cd "${LOCAL_GFXSTREAM_PATH}" && pwd)"
else
    HOST_GFXSTREAM_DIR="${BUILD_DIR}/host_gfxstream"
fi
HOST_GFXSTREAM_BUILD_DIR="${BUILD_DIR}/host_gfxstream/host-build"
HOST_GFXSTREAM_INSTALL_DIR="${BUILD_DIR}/host_gfxstream/install"

GPU_SOCKET_PATH="${KUMQUAT_GPU_SOCKET_PATH:-/tmp/kumquat-gpu-0}"

mkdir -p "${BUILD_DIR}"

# 1. Clone and build guest gfxstream Mesa driver.
if [ ! -d "${GUEST_GFXSTREAM_DIR}" ]; then
    echo "${GUEST_GFXSTREAM_DIR} not found. Cloning..."
    echo ""

    git clone https://gitlab.freedesktop.org/mesa/mesa.git \
        "${GUEST_GFXSTREAM_DIR}"

    echo ""
    echo "Cloning succeeded!"
else
    echo "Found existing ${GUEST_GFXSTREAM_DIR} directory. Reusing."
fi

if [ ! -d "${GUEST_GFXSTREAM_BUILD_DIR}" ]; then
    echo "${GUEST_GFXSTREAM_BUILD_DIR} not found. Performing meson setup..."
    echo ""

    meson setup "${GUEST_GFXSTREAM_BUILD_DIR}" "${GUEST_GFXSTREAM_DIR}" \
        -Dvulkan-drivers="gfxstream" \
        -Dgallium-drivers="" \
        -Dvirtgpu_kumquat=true \
        -Dopengl=false \
        -Drust_std=2021

    echo ""
    echo "Meson setup succeeded!"
else
    echo "Found existing ${GUEST_GFXSTREAM_BUILD_DIR} directory. Reusing."
fi

echo "Building Gfxstream guest driver..."
echo ""

ninja -C "${GUEST_GFXSTREAM_BUILD_DIR}"

echo ""
echo "Gfxstream guest driver build succeeded!"

# 2. Clone (if needed) and build host gfxstream backend.
if [ -n "${LOCAL_GFXSTREAM_PATH}" ]; then
    echo "Using local Gfxstream source directory: ${HOST_GFXSTREAM_DIR}"
elif [ ! -d "${HOST_GFXSTREAM_DIR}" ]; then
    echo "${HOST_GFXSTREAM_DIR} not found. Cloning..."
    echo ""

    git clone https://github.com/google/gfxstream "${HOST_GFXSTREAM_DIR}"

    echo ""
    echo "Cloning succeeded!"
else
    echo "Found existing ${HOST_GFXSTREAM_DIR} directory. Reusing."
fi

if [ -f "${HOST_GFXSTREAM_BUILD_DIR}/build.ninja" ] && \
   ! grep -Fq "${HOST_GFXSTREAM_DIR}" "${HOST_GFXSTREAM_BUILD_DIR}/build.ninja"; then
    echo "Host build directory was configured for a different source path. Reconfiguring..."
    rm -rf "${HOST_GFXSTREAM_BUILD_DIR}"
fi

if [ ! -d "${HOST_GFXSTREAM_BUILD_DIR}" ]; then
    echo "${HOST_GFXSTREAM_BUILD_DIR} not found. Performing meson setup..."
    echo ""

    meson setup "${HOST_GFXSTREAM_BUILD_DIR}" "${HOST_GFXSTREAM_DIR}" \
        --prefix="${HOST_GFXSTREAM_INSTALL_DIR}" \
        --libdir=lib

    echo ""
    echo "Meson setup succeeded!"
else
    echo "Found existing ${HOST_GFXSTREAM_BUILD_DIR} directory. Reusing."
fi

echo "Building Gfxstream host library..."
echo ""

ninja -C "${HOST_GFXSTREAM_BUILD_DIR}"
meson install -C "${HOST_GFXSTREAM_BUILD_DIR}"

echo ""
echo "Gfxstream host library build succeeded!"

# 3. Build broccolini and kumquat against the locally built host_gfxstream.
echo "Building Kumquat with Broccolini..."
echo ""

export PKG_CONFIG_PATH="${HOST_GFXSTREAM_INSTALL_DIR}/lib/pkgconfig:${HOST_GFXSTREAM_INSTALL_DIR}/lib/x86_64-linux-gnu/pkgconfig:${PKG_CONFIG_PATH:-}"
export GFXSTREAM_PATH="${HOST_GFXSTREAM_INSTALL_DIR}/lib"
export LD_LIBRARY_PATH="${HOST_GFXSTREAM_INSTALL_DIR}/lib:${HOST_GFXSTREAM_BUILD_DIR}/host:${LD_LIBRARY_PATH:-}"

cargo build \
    --manifest-path "${RUTABAGA_DIR}/broccolini/Cargo.toml" \
    --features=gfxstream

cargo build \
    --manifest-path "${RUTABAGA_DIR}/kumquat/server/Cargo.toml" \
    --features=gfxstream

echo ""
echo "Kumquat with Broccolini build succeeded!"

KUMQUAT_BIN="${RUTABAGA_DIR}/target/debug/kumquat"
BROCCOLINI_BIN="${RUTABAGA_DIR}/target/debug/broccolini"
REMOTE_CAPSET_NAMES="${KUMQUAT_REMOTE_CAPSET_NAMES:-gfxstream-vulkan}"
RENDERER_FEATURES="${KUMQUAT_RENDERER_FEATURES:-Vulkan:enabled,ExternalBlob:enabled,VulkanSnapshots:disabled,VulkanExternalMemoryMode:DmaBuf}"

# 4. Launch kumquat with broccolini if the socket does not already exist.
if [ ! -S "${GPU_SOCKET_PATH}" ]; then
    echo "Socket ${GPU_SOCKET_PATH} not found; launching kumquat with broccolini..."
    "${KUMQUAT_BIN}" \
        --gpu-socket-path="${GPU_SOCKET_PATH}" \
        --remote-capset-names="${REMOTE_CAPSET_NAMES}" \
        --broccolini-path="${BROCCOLINI_BIN}" \
        --renderer-features="${RENDERER_FEATURES}" &

    for _ in $(seq 1 50); do
        if [ -S "${GPU_SOCKET_PATH}" ]; then
            break
        fi
        sleep 0.1
    done

    if [ ! -S "${GPU_SOCKET_PATH}" ]; then
        echo "Error: timed out waiting for ${GPU_SOCKET_PATH} to be created by kumquat." >&2
        exit 1
    fi
else
    echo "Existing Kumquat socket found (${GPU_SOCKET_PATH}). Reusing..."
    echo ""
fi

# 5. Launch the target command with the guest gfxstream driver environment variables.
ARCH="$(uname -m)"
ICD_JSON="${GUEST_GFXSTREAM_BUILD_DIR}/src/gfxstream/guest/vulkan/gfxstream_vk_devenv_icd.${ARCH}.json"

export MESA_LOADER_DRIVER_OVERRIDE=zink
export VIRTGPU_KUMQUAT=1
unset VK_ICD_FILENAMES
export VK_DRIVER_FILES="${ICD_JSON}"

exec "$@"
