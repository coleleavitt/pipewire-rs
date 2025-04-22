// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

//! PipeWire Loop implementation
//!
//! This module provides both regular Loops and DataLoops, which can be configured
//! with various properties such as CPU affinity, realtime priority, and class.

mod ref_;
mod owned;
mod traits;
mod sources;
mod data_loop;

pub use ref_::LoopRef;
pub use owned::{Loop, WeakLoop, LoopBuilder};
pub use traits::{IsLoopRc, IsSource};
pub use sources::{
    IoSource,
    IdleSource,
    SignalSource,
    EventSource,
    TimerSource,
};

// Explicitly re-export from pw_sys instead of defining our own
pub use pw_sys::pw_data_loop;
