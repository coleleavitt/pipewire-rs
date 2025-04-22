// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use bitflags::bitflags;

bitflags! {
    /// Extra flags that can be used in stream connections
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct StreamFlags: pw_sys::pw_stream_flags {
        const AUTOCONNECT = pw_sys::pw_stream_flags_PW_STREAM_FLAG_AUTOCONNECT;
        const INACTIVE = pw_sys::pw_stream_flags_PW_STREAM_FLAG_INACTIVE;
        const MAP_BUFFERS = pw_sys::pw_stream_flags_PW_STREAM_FLAG_MAP_BUFFERS;
        const DRIVER = pw_sys::pw_stream_flags_PW_STREAM_FLAG_DRIVER;
        const RT_PROCESS = pw_sys::pw_stream_flags_PW_STREAM_FLAG_RT_PROCESS;
        const NO_CONVERT = pw_sys::pw_stream_flags_PW_STREAM_FLAG_NO_CONVERT;
        const EXCLUSIVE = pw_sys::pw_stream_flags_PW_STREAM_FLAG_EXCLUSIVE;
        const DONT_RECONNECT = pw_sys::pw_stream_flags_PW_STREAM_FLAG_DONT_RECONNECT;
        const ALLOC_BUFFERS = pw_sys::pw_stream_flags_PW_STREAM_FLAG_ALLOC_BUFFERS;
        // Add unconditionally (remove feature gate)
        const TRIGGER = pw_sys::pw_stream_flags_PW_STREAM_FLAG_TRIGGER;
        const ASYNC = pw_sys::pw_stream_flags_PW_STREAM_FLAG_ASYNC;

        // Add new flags from PipeWire 1.2
        const EARLY_PROCESS = pw_sys::pw_stream_flags_PW_STREAM_FLAG_EARLY_PROCESS;
        const RT_TRIGGER_DONE = pw_sys::pw_stream_flags_PW_STREAM_FLAG_RT_TRIGGER_DONE;
    }
}
