// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{
    ptr::{self, NonNull},
    rc::{Rc, Weak},
};
use std::ops::Deref;
use super::{ref_::LoopRef, traits::IsLoopRc};
use crate::Error;

/// An owned PipeWire loop
#[derive(Clone, Debug)]
pub struct Loop {
    inner: Rc<LoopInner>,
}

impl Loop {
    /// Create a new [`Loop`].
    pub fn new(properties: Option<&spa::utils::dict::DictRef>) -> Result<Self, Error> {
        // This is a potential "entry point" to the library, so we need to ensure it is initialized.
        crate::init();

        unsafe {
            let props = properties
                .map_or(ptr::null(), |props| props.as_raw())
                .cast_mut();
            let l = pw_sys::pw_loop_new(props);
            let ptr = ptr::NonNull::new(l).ok_or(Error::CreationFailed)?;
            Ok(Self::from_raw(ptr))
        }
    }

    /// Create a new loop from a raw [`pw_loop`](`pw_sys::pw_loop`), taking ownership of it.
    ///
    /// # Safety
    /// The provided pointer must point to a valid, well aligned [`pw_loop`](`pw_sys::pw_loop`).
    ///
    /// The raw loop should not be manually destroyed or moved, as the new [`Loop`] takes ownership of it.
    pub unsafe fn from_raw(ptr: NonNull<pw_sys::pw_loop>) -> Self {
        Self {
            inner: Rc::new(LoopInner::from_raw(ptr)),
        }
    }

    /// Create a weak reference to this loop that can be upgraded later
    pub fn downgrade(&self) -> WeakLoop {
        let weak = Rc::downgrade(&self.inner);
        WeakLoop { weak }
    }
}

// Safety: The inner pw_loop is guaranteed to remain valid while any clone of the `Loop` is held,
//         because we use an internal Rc to keep it alive.
unsafe impl IsLoopRc for Loop {}

impl std::ops::Deref for Loop {
    type Target = LoopRef;

    fn deref(&self) -> &Self::Target {
        let loop_ = self.inner.ptr.as_ptr();
        unsafe { &*(loop_.cast::<LoopRef>()) }
    }
}

impl std::convert::AsRef<LoopRef> for Loop {
    fn as_ref(&self) -> &LoopRef {
        self.deref()
    }
}

/// A weak reference to a PipeWire loop
pub struct WeakLoop {
    weak: Weak<LoopInner>,
}

impl WeakLoop {
    /// Try to upgrade this weak reference to a full Loop
    pub fn upgrade(&self) -> Option<Loop> {
        self.weak.upgrade().map(|inner| Loop { inner })
    }
}

#[derive(Debug)]
struct LoopInner {
    ptr: ptr::NonNull<pw_sys::pw_loop>,
}

impl LoopInner {
    pub unsafe fn from_raw(ptr: NonNull<pw_sys::pw_loop>) -> Self {
        Self { ptr }
    }
}

impl Drop for LoopInner {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_loop_destroy(self.ptr.as_ptr()) }
    }
}
