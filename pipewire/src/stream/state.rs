// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{ffi::CStr, os};

/// Represents the current state of a stream.
#[derive(Debug, PartialEq)]
pub enum StreamState {
    Error(String),
    Unconnected,
    Connecting,
    Paused,
    Streaming,
}

impl StreamState {
    pub(crate) fn from_raw(state: pw_sys::pw_stream_state, error: *const os::raw::c_char) -> Self {
        match state {
            pw_sys::pw_stream_state_PW_STREAM_STATE_UNCONNECTED => StreamState::Unconnected,
            pw_sys::pw_stream_state_PW_STREAM_STATE_CONNECTING => StreamState::Connecting,
            pw_sys::pw_stream_state_PW_STREAM_STATE_PAUSED => StreamState::Paused,
            pw_sys::pw_stream_state_PW_STREAM_STATE_STREAMING => StreamState::Streaming,
            _ => {
                let error = if error.is_null() {
                    "".to_string()
                } else {
                    unsafe { CStr::from_ptr(error).to_string_lossy().to_string() }
                };

                StreamState::Error(error)
            }
        }
    }
}
