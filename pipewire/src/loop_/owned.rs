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

    /// Create a new builder for configuring a [`Loop`] with specific properties.
    pub fn builder() -> LoopBuilder {
        LoopBuilder::default()
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

/// A builder for creating a configured [`Loop`]
#[derive(Default)]
pub struct LoopBuilder {
    name: Option<String>,
    class: Option<String>,
    cpu_affinity: Option<Vec<u32>>,
    rt_priority: Option<i32>,
    properties: Vec<(String, String)>,
}

impl LoopBuilder {
    /// Set the name of the loop
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the class of the loop
    ///
    /// PipeWire 1.2 introduced support for loop classes like "data.rt"
    /// which affect the scheduling behavior of the loop.
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    /// Set the CPU affinity for this loop
    ///
    /// This determines which CPU cores the loop thread will run on.
    pub fn cpu_affinity(mut self, cpu_ids: impl Into<Vec<u32>>) -> Self {
        self.cpu_affinity = Some(cpu_ids.into());
        self
    }

    /// Set the realtime priority for this loop
    pub fn rt_priority(mut self, priority: i32) -> Self {
        self.rt_priority = Some(priority);
        self
    }

    /// Add a custom property to the loop
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push((key.into(), value.into()));
        self
    }

    /// Build the loop with the configured properties
    pub fn build(self) -> Result<Loop, Error> {
        // Create a base loop
        let loop_ = Loop::new(None)?;

        // Set name if specified
        if let Some(name) = self.name {
            loop_.set_name(&name)?;
        }

        // Set class if specified
        if let Some(class) = self.class {
            loop_.set_class(&class)?;
        }

        // Set CPU affinity if specified
        if let Some(cpu_ids) = self.cpu_affinity {
            loop_.set_cpu_affinity(&cpu_ids)?;
        }

        // Set RT priority if specified
        if let Some(priority) = self.rt_priority {
            loop_.set_rt_priority(priority)?;
        }

        // Set custom properties
        for (key, value) in self.properties {
            loop_.set_property(&key, &value)?;
        }

        Ok(loop_)
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
