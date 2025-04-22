// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

pub mod audio;
pub mod format;
pub mod format_utils;
pub mod video;

use std::ffi::CStr;
use std::fmt::Debug;
use pipewire_sys::pw_buffer;
/// A wrapper around spa_param_type
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ParamType(pub spa_sys::spa_param_type);

#[allow(non_upper_case_globals)]
impl ParamType {
    pub const Invalid: Self = Self(spa_sys::SPA_PARAM_Invalid);
    pub const PropInfo: Self = Self(spa_sys::SPA_PARAM_PropInfo);
    pub const Props: Self = Self(spa_sys::SPA_PARAM_Props);
    pub const EnumFormat: Self = Self(spa_sys::SPA_PARAM_EnumFormat);
    pub const Format: Self = Self(spa_sys::SPA_PARAM_Format);
    pub const Buffers: Self = Self(spa_sys::SPA_PARAM_Buffers);
    pub const Meta: Self = Self(spa_sys::SPA_PARAM_Meta);
    pub const IO: Self = Self(spa_sys::SPA_PARAM_IO);
    pub const EnumProfile: Self = Self(spa_sys::SPA_PARAM_EnumProfile);
    pub const Profile: Self = Self(spa_sys::SPA_PARAM_Profile);
    pub const EnumPortConfig: Self = Self(spa_sys::SPA_PARAM_EnumPortConfig);
    pub const PortConfig: Self = Self(spa_sys::SPA_PARAM_PortConfig);
    pub const EnumRoute: Self = Self(spa_sys::SPA_PARAM_EnumRoute);
    pub const Route: Self = Self(spa_sys::SPA_PARAM_Route);
    pub const Control: Self = Self(spa_sys::SPA_PARAM_Control);
    pub const Latency: Self = Self(spa_sys::SPA_PARAM_Latency);
    pub const ProcessLatency: Self = Self(spa_sys::SPA_PARAM_ProcessLatency);

    /// Obtain a [`ParamType`] from a raw `spa_param_type` variant.
    pub fn from_raw(raw: spa_sys::spa_param_type) -> Self {
        Self(raw)
    }

    /// Get the raw [`spa_sys::spa_param_type`] representing this `ParamType`.
    pub fn as_raw(&self) -> spa_sys::spa_param_type {
        self.0
    }
}

impl Debug for ParamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c_str = unsafe {
            let c_buf =
                spa_sys::spa_debug_type_find_short_name(spa_sys::spa_type_param, self.as_raw());
            if c_buf.is_null() {
                return f.write_str("Unknown");
            }
            CStr::from_ptr(c_buf)
        };
        let name = format!("ParamType::{}", c_str.to_string_lossy());
        f.write_str(&name)
    }
}

bitflags::bitflags! {
    /// Flags for ParamInfo
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct ParamInfoFlags: u32 {
        const SERIAL = 1<<0;
        const READ   = 1<<1;
        const WRITE  = 1<<2;
        const READWRITE = Self::READ.bits() | Self::WRITE.bits();
    }
}

/// A transparent wrapper around a spa_param_info.
#[repr(transparent)]
pub struct ParamInfo(pub(crate) spa_sys::spa_param_info);

impl ParamInfo {
    /// Get the param id
    pub fn id(&self) -> ParamType {
        ParamType::from_raw(self.0.id)
    }

    /// Get the param flags
    pub fn flags(&self) -> ParamInfoFlags {
        ParamInfoFlags::from_bits_truncate(self.0.flags)
    }
}

impl Debug for ParamInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParamInfo")
            .field("id", &self.id())
            .field("flags", &self.flags())
            .finish()
    }
}

/// A wrapper around spa metadata types
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MetaType(pub u32);

#[allow(non_upper_case_globals)]
impl MetaType {
    pub const Invalid: Self = Self(spa_sys::SPA_META_Invalid);
    pub const Header: Self = Self(spa_sys::SPA_META_Header);
    pub const VideoCrop: Self = Self(spa_sys::SPA_META_VideoCrop);
    pub const VideoDamage: Self = Self(spa_sys::SPA_META_VideoDamage);
    pub const Bitmap: Self = Self(spa_sys::SPA_META_Bitmap);
    pub const Cursor: Self = Self(spa_sys::SPA_META_Cursor);
    pub const Control: Self = Self(spa_sys::SPA_META_Control);
    pub const Busy: Self = Self(spa_sys::SPA_META_Busy);
    pub const VideoTransform: Self = Self(spa_sys::SPA_META_VideoTransform);
    pub const SyncTimeline: Self = Self(spa_sys::SPA_META_SyncTimeline);

    /// Obtain a [`MetaType`] from a raw spa metadata type value.
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the raw value representing this `MetaType`.
    pub fn as_raw(&self) -> u32 {
        self.0
    }
}

impl Debug for MetaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match *self {
            Self::Invalid => "MetaType::Invalid",
            Self::Header => "MetaType::Header",
            Self::VideoCrop => "MetaType::VideoCrop",
            Self::VideoDamage => "MetaType::VideoDamage",
            Self::Bitmap => "MetaType::Bitmap",
            Self::Cursor => "MetaType::Cursor",
            Self::Control => "MetaType::Control",
            Self::Busy => "MetaType::Busy",
            Self::VideoTransform => "MetaType::VideoTransform",
            Self::SyncTimeline => "MetaType::SyncTimeline",
            _ => return f.write_str("MetaType::Unknown"),
        };
        f.write_str(name)
    }
}

pub trait TimelineManager {
    async fn set_acquire_point(&self, point: u64) -> Result<(), anyhow::Error>;
    async fn signal(&self, point: u64) -> Result<(), anyhow::Error>;
    async fn is_signaled(&self, point: u64) -> Result<bool, anyhow::Error>;
    async fn wait_for_available(&self) -> Result<(), anyhow::Error>;
    async fn queue_buffer(&self, buffer: *mut pw_buffer);
}

/// A struct to manage the timeline for explicit synchronization
pub struct Timeline {
    fd: i32,
}

impl Timeline {
    pub fn new(fd: i32) -> Self {
        Timeline { fd }
    }
}
