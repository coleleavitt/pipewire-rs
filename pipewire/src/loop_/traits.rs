// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use super::ref_::LoopRef;

/// Trait implemented by objects that implement a `pw_loop` and are reference counted in some way.
///
/// # Safety
///
/// The `LoopRef` returned by the implementation of `AsRef<LoopRef>` must remain valid as long as any clone
/// of the trait implementor is still alive.
pub unsafe trait IsLoopRc: Clone + AsRef<LoopRef> + 'static {}

/// Trait for objects that represent a source in a PipeWire loop
pub trait IsSource {
    /// Return a valid pointer to a raw `spa_source`.
    fn as_ptr(&self) -> *mut spa_sys::spa_source;
}
