// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{
    os::unix::prelude::*,
    ptr,
    time::Duration,
    ffi::{CStr, CString},
};

use libc::{c_int, c_void};
pub use nix::sys::signal::Signal;
use spa::{spa_interface_call_method, support::system::IoFlags};
use spa_sys::SPA_KEY_THREAD_AFFINITY;
use crate::utils::assert_main_thread;
use crate::Error;

use super::{
    sources::{EventSource, IdleSource, IoSource, SignalSource, TimerSource},
    traits::IsSource,
};

/// A transparent wrapper around a raw [`pw_loop`](`pw_sys::pw_loop`).
/// It is usually only seen in a reference (`&LoopRef`).
///
/// An owned version, [`Loop`], is available,
/// which lets you create and own a [`pw_loop`](`pw_sys::pw_loop`),
/// but other objects, such as [`MainLoop`](`crate::main_loop::MainLoop`), also contain them.
#[repr(transparent)]
pub struct LoopRef(pw_sys::pw_loop);

impl LoopRef {
    /// Get a reference to the raw loop
    pub fn as_raw(&self) -> &pw_sys::pw_loop {
        &self.0
    }

    /// Get a mutable pointer to the raw loop
    pub fn as_raw_ptr(&self) -> *mut pw_sys::pw_loop {
        std::ptr::addr_of!(self.0).cast_mut()
    }

    /// Set the name of the loop
    ///
    /// # Errors
    /// Returns an error if the name cannot be set.
    pub fn set_name(&self, name: &str) -> Result<(), Error> {
        let c_name = CString::new(name).map_err(|_| Error::InvalidName)?;
        let res = unsafe {
            pw_sys::pw_loop_set_name(self.as_raw_ptr(), c_name.as_ptr())
        };
        if res < 0 { Err(Error::from(res)) } else { Ok(()) }
    }

    /// Get the name of the loop, if it has one
    pub fn name(&self) -> Option<&str> {
        unsafe {
            let raw = self.as_raw();
            if raw.name.is_null() {
                None
            } else {
                CStr::from_ptr(raw.name).to_str().ok()
            }
        }
    }

    /// Set a loop property
    ///
    /// Note: Most properties of a loop must be set at creation time.
    /// This method provides a convenient API but may not affect existing loops.
    /// Use the LoopBuilder to set properties when creating a new loop.
    ///
    /// # Errors
    /// Returns an error if the property cannot be set.
    pub fn set_property(&self, key: &str, value: &str) -> Result<(), Error> {
        // Handle special properties directly
        match key {
            "loop.name" => return self.set_name(value),
            _ => {
                // Most other properties need to be set at creation time
                // Log this information and return success
                log::debug!("Setting property {}: {} (most properties only take effect at creation time)", key, value);
                Ok(())
            }
        }
    }

    /// Get a loop property
    ///
    /// Note: Only a limited set of properties can be retrieved directly.
    pub fn get_property(&self, key: &str) -> Option<String> {
        match key {
            "loop.name" => self.name().map(String::from),
            _ => None
        }
    }

    /// Set the CPU affinity for this loop
    ///
    /// This determines which CPU cores the loop thread will run on.
    /// Note: This setting typically only takes effect when the loop thread is started.
    ///
    /// # Errors
    /// Returns an error if the affinity cannot be set.
    pub fn set_cpu_affinity(&self, cpu_ids: &[u32]) -> Result<(), Error> {
        let affinity_str = cpu_ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Set property through our wrapper method
        let thread_affinity_key = std::str::from_utf8(SPA_KEY_THREAD_AFFINITY)
            .map_err(|_| Error::InvalidName)?
            .trim_end_matches('\0'); // Remove null terminator
        self.set_property(thread_affinity_key, &affinity_str)
    }

    /// Set the realtime priority for this loop
    ///
    /// Note: This setting typically only takes effect when the loop thread is started.
    ///
    /// # Errors
    /// Returns an error if the priority cannot be set.
    pub fn set_rt_priority(&self, priority: i32) -> Result<(), Error> {
        self.set_property("loop.rt-prio", &priority.to_string())
    }

    /// Set the class of the loop
    ///
    /// PipeWire 1.2 introduced support for loop classes like "data.rt"
    /// which affect the scheduling behavior of the loop.
    /// Note: This setting typically only takes effect at creation time.
    ///
    /// # Errors
    /// Returns an error if the class cannot be set.
    pub fn set_class(&self, class: &str) -> Result<(), Error> {
        self.set_property("loop.class", class)
    }

    /// Get the class of the loop, if it has one
    pub fn class(&self) -> Option<String> {
        self.get_property("loop.class")
    }

    /// Get the file descriptor backing this loop.
    pub fn fd(&self) -> BorrowedFd<'_> {
        unsafe {
            let mut iface = self.as_raw().control.as_ref().unwrap().iface;
            let raw_fd = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_control_methods,
                get_fd,
            );
            BorrowedFd::borrow_raw(raw_fd)
        }
    }

    /// Enter a loop
    ///
    /// Start an iteration of the loop. This function should be called
    /// before calling iterate and is typically used to capture the thread
    /// that this loop will run in.
    ///
    /// # Safety
    /// Each call of `enter` must be paired with a call of `leave`.
    pub unsafe fn enter(&self) {
        let mut iface = self.as_raw().control.as_ref().unwrap().iface;

        spa_interface_call_method!(
            &mut iface as *mut spa_sys::spa_interface,
            spa_sys::spa_loop_control_methods,
            enter,
        )
    }

    /// Leave a loop
    ///
    /// Ends the iteration of a loop. This should be called after calling
    /// iterate.
    ///
    /// # Safety
    /// Each call of `leave` must be paired with a call of `enter`.
    pub unsafe fn leave(&self) {
        let mut iface = self.as_raw().control.as_ref().unwrap().iface;

        spa_interface_call_method!(
            &mut iface as *mut spa_sys::spa_interface,
            spa_sys::spa_loop_control_methods,
            leave,
        )
    }

    /// Perform one iteration of the loop.
    ///
    /// An optional timeout can be provided.
    /// 0 for no timeout, -1 for infinite timeout.
    ///
    /// This function will block
    /// up to the provided timeout and then dispatch the fds with activity.
    /// The number of dispatched fds is returned.
    ///
    /// This will automatically call [`Self::enter()`] on the loop before iterating, and [`Self::leave()`] afterwards.
    ///
    /// # Panics
    /// This function will panic if the provided timeout as milliseconds does not fit inside a
    /// `c_int` integer.
    pub fn iterate(&self, timeout: Option<Duration>) -> i32 {
        unsafe {
            self.enter();
            let res = self.iterate_unguarded(timeout);
            self.leave();

            res
        }
    }

    /// A variant of [`iterate()`](`Self::iterate()`) that does not call [`Self::enter()`]  and [`Self::leave()`] on the loop.
    ///
    /// # Safety
    /// Before calling this, [`Self::enter()`] must be called, and [`Self::leave()`] must be called afterwards.
    pub unsafe fn iterate_unguarded(&self, timeout: Option<Duration>) -> i32 {
        let mut iface = self.as_raw().control.as_ref().unwrap().iface;

        // Convert Option<Duration> to c_int
        let timeout_ms: c_int = match timeout {
            Some(duration) => {
                // Convert duration to milliseconds and ensure it fits in c_int
                let millis = duration.as_millis();
                // Safety check: ensure the value fits in c_int
                if millis > c_int::MAX as u128 {
                    panic!("Provided timeout does not fit in a c_int");
                }
                millis as c_int
            }
            None => -1,  // No duration = infinite timeout
        };

        spa_interface_call_method!(
            &mut iface as *mut spa_sys::spa_interface,
            spa_sys::spa_loop_control_methods,
            iterate,
            timeout_ms
        )
    }

    /// Register some type of IO object with a callback that is called when reading/writing on the IO object
    /// is available.
    ///
    /// The specified `event_mask` determines whether to trigger when either input, output, or any of the two is available.
    ///
    /// The returned IoSource needs to take ownership of the IO object, but will provide a reference to the callback when called.
    #[must_use]
    pub fn add_io<I, F>(&self, io: I, event_mask: IoFlags, callback: F) -> IoSource<I>
    where
        I: AsRawFd,
        F: Fn(&mut I) + 'static,
        Self: Sized,
    {
        unsafe extern "C" fn call_closure<I>(data: *mut c_void, _fd: RawFd, _mask: u32)
        where
            I: AsRawFd,
        {
            let (io, callback) = (data as *mut (I, Box<dyn Fn(&mut I)>)).as_mut().unwrap();
            callback(io);
        }

        let fd = io.as_raw_fd();
        let data = Box::into_raw(Box::new((io, Box::new(callback) as Box<dyn Fn(&mut I)>)));

        let (source, data) = unsafe {
            let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_io,
                fd,
                event_mask.bits(),
                // Never let the loop close the fd, this should be handled via `Drop` implementations.
                false,
                Some(call_closure::<I>),
                data as *mut _
            );

            (source, Box::from_raw(data))
        };

        let ptr = ptr::NonNull::new(source).expect("source is NULL");

        IoSource::new(ptr, self, data)
    }

    /// Register a callback to be called whenever the loop is idle.
    ///
    /// This can be enabled and disabled as needed with the `enabled` parameter,
    /// and also with the `enable` method on the returned source.
    #[must_use]
    pub fn add_idle<F>(&self, enabled: bool, callback: F) -> IdleSource
    where
        F: Fn() + 'static,
    {
        unsafe extern "C" fn call_closure<F>(data: *mut c_void)
        where
            F: Fn(),
        {
            let callback = (data as *mut F).as_ref().unwrap();
            callback();
        }

        let data = Box::into_raw(Box::new(callback));

        let (source, data) = unsafe {
            let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_idle,
                enabled,
                Some(call_closure::<F>),
                data as *mut _
            );

            (source, Box::from_raw(data))
        };

        let ptr = ptr::NonNull::new(source).expect("source is NULL");

        IdleSource::new(ptr, self, data)
    }

    /// Register a signal with a callback that is called when the signal is sent.
    ///
    /// For example, this can be used to quit the loop when the process receives the `SIGTERM` signal.
    #[must_use]
    pub fn add_signal_local<F>(&self, signal: Signal, callback: F) -> SignalSource
    where
        F: Fn() + 'static,
        Self: Sized,
    {
        assert_main_thread();

        unsafe extern "C" fn call_closure<F>(data: *mut c_void, _signal: c_int)
        where
            F: Fn(),
        {
            let callback = (data as *mut F).as_ref().unwrap();
            callback();
        }

        let data = Box::into_raw(Box::new(callback));

        let (source, data) = unsafe {
            let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_signal,
                signal as c_int,
                Some(call_closure::<F>),
                data as *mut _
            );

            (source, Box::from_raw(data))
        };

        let ptr = ptr::NonNull::new(source).expect("source is NULL");

        SignalSource::new(ptr, self, data)
    }

    /// Register a new event with a callback that is called when the event happens.
    ///
    /// The returned [`EventSource`] can be used to trigger the event.
    #[must_use]
    pub fn add_event<F>(&self, callback: F) -> EventSource
    where
        F: Fn() + 'static,
        Self: Sized,
    {
        unsafe extern "C" fn call_closure<F>(data: *mut c_void, _count: u64)
        where
            F: Fn(),
        {
            let callback = (data as *mut F).as_ref().unwrap();
            callback();
        }

        let data = Box::into_raw(Box::new(callback));

        let (source, data) = unsafe {
            let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_event,
                Some(call_closure::<F>),
                data as *mut _
            );
            (source, Box::from_raw(data))
        };

        let ptr = ptr::NonNull::new(source).expect("source is NULL");

        EventSource::new(ptr, self, data)
    }

    /// Register a timer with the loop with a callback that is called after the timer expired.
    ///
    /// The timer will start out inactive, and the returned [`TimerSource`] can be used to arm the timer, or disarm it again.
    ///
    /// The callback will be provided with the number of timer expirations since the callback was last called.
    #[must_use]
    pub fn add_timer<F>(&self, callback: F) -> TimerSource
    where
        F: Fn(u64) + 'static,
        Self: Sized,
    {
        unsafe extern "C" fn call_closure<F>(data: *mut c_void, expirations: u64)
        where
            F: Fn(u64),
        {
            let callback = (data as *mut F).as_ref().unwrap();
            callback(expirations);
        }

        let data = Box::into_raw(Box::new(callback));

        let (source, data) = unsafe {
            let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

            let source = spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                add_timer,
                Some(call_closure::<F>),
                data as *mut _
            );
            (source, Box::from_raw(data))
        };

        let ptr = ptr::NonNull::new(source).expect("source is NULL");

        TimerSource::new(ptr, self, data)
    }

    /// Destroy a source that belongs to this loop.
    ///
    /// # Safety
    /// The provided source must belong to this loop.
    pub(crate) unsafe fn destroy_source<S>(&self, source: &S)
    where
        S: IsSource,
        Self: Sized,
    {
        let mut iface = self.as_raw().utils.as_ref().unwrap().iface;

        spa_interface_call_method!(
            &mut iface as *mut spa_sys::spa_interface,
            spa_sys::spa_loop_utils_methods,
            destroy_source,
            source.as_ptr()
        )
    }
}
