// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#[macro_export]
macro_rules! ioctl_write_ptr {
    ($name:ident, $ioty:expr, $nr:expr, $ty:ty) => {
        pub unsafe fn $name(fd: std::os::fd::BorrowedFd, data: &$ty) -> std::io::Result<()> {
            const OPCODE: rustix::ioctl::Opcode =
                rustix::ioctl::opcode::write::<$ty>($ioty as u8, $nr as u8);
            Ok(rustix::ioctl::ioctl(
                fd,
                rustix::ioctl::Setter::<OPCODE, $ty>::new(*data),
            )?)
        }
    };
}

#[macro_export]
macro_rules! ioctl_readwrite {
    ($name:ident, $ioty:expr, $nr:expr, $ty:ty) => {
        pub unsafe fn $name(fd: std::os::fd::BorrowedFd, data: &mut $ty) -> std::io::Result<()> {
            const OPCODE: rustix::ioctl::Opcode =
                rustix::ioctl::opcode::read_write::<$ty>($ioty as u8, $nr as u8);
            Ok(rustix::ioctl::ioctl(
                fd,
                rustix::ioctl::Updater::<OPCODE, $ty>::new(data),
            )?)
        }
    };
}
