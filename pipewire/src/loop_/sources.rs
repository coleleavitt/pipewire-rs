// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{
    convert::TryInto,
    os::unix::prelude::*,
    ptr,
    time::Duration
};
use spa::spa_interface_call_method;
use spa::utils::result::SpaResult;

use super::{ref_::LoopRef, traits::IsSource};

type IoSourceData<I> = (I, Box<dyn Fn(&mut I) + 'static>);

/// A source that can be used to react to IO events.
///
/// This source can be obtained by calling [`add_io`](`LoopRef::add_io`) on a loop, registering a callback to it.
pub struct IoSource<'l, I>
where
    I: AsRawFd,
{
    ptr: ptr::NonNull<spa_sys::spa_source>,
    loop_: &'l LoopRef,
    // Store data wrapper to prevent leak
    _data: Box<IoSourceData<I>>,
}

impl<'l, I> IoSource<'l, I>
where
    I: AsRawFd,
{
    pub(crate) fn new(
        ptr: ptr::NonNull<spa_sys::spa_source>,
        loop_: &'l LoopRef,
        data: Box<IoSourceData<I>>,
    ) -> Self {
        Self {
            ptr,
            loop_,
            _data: data,
        }
    }
}

impl<'l, I> IsSource for IoSource<'l, I>
where
    I: AsRawFd,
{
    fn as_ptr(&self) -> *mut spa_sys::spa_source {
        self.ptr.as_ptr()
    }
}

impl<'l, I> Drop for IoSource<'l, I>
where
    I: AsRawFd,
{
    fn drop(&mut self) {
        unsafe { self.loop_.destroy_source(self) }
    }
}

/// A source that can be used to have a callback called when the loop is idle.
///
/// This source can be obtained by calling [`add_idle`](`LoopRef::add_idle`) on a loop, registering a callback to it.
pub struct IdleSource<'l> {
    ptr: ptr::NonNull<spa_sys::spa_source>,
    loop_: &'l LoopRef,
    // Store data wrapper to prevent leak
    _data: Box<dyn Fn() + 'static>,
}

impl<'l> IdleSource<'l> {
    pub(crate) fn new(
        ptr: ptr::NonNull<spa_sys::spa_source>,
        loop_: &'l LoopRef,
        data: Box<dyn Fn() + 'static>,
    ) -> Self {
        Self {
            ptr,
            loop_,
            _data: data,
        }
    }

    /// Set the source as enabled or disabled, allowing or preventing the callback from being called.
    pub fn enable(&self, enable: bool) {
        unsafe {
            let mut iface = self.loop_.as_raw().utils.as_ref().unwrap().iface;

            spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                enable_idle,
                self.as_ptr(),
                enable
            );
        }
    }
}

impl<'l> IsSource for IdleSource<'l> {
    fn as_ptr(&self) -> *mut spa_sys::spa_source {
        self.ptr.as_ptr()
    }
}

impl<'l> Drop for IdleSource<'l> {
    fn drop(&mut self) {
        unsafe { self.loop_.destroy_source(self) }
    }
}

/// A source that can be used to react to signals.
///
/// This source can be obtained by calling [`add_signal_local`](`LoopRef::add_signal_local`) on a loop, registering a callback to it.
pub struct SignalSource<'l> {
    ptr: ptr::NonNull<spa_sys::spa_source>,
    loop_: &'l LoopRef,
    // Store data wrapper to prevent leak
    _data: Box<dyn Fn() + 'static>,
}

impl<'l> SignalSource<'l> {
    pub(crate) fn new(
        ptr: ptr::NonNull<spa_sys::spa_source>,
        loop_: &'l LoopRef,
        data: Box<dyn Fn() + 'static>,
    ) -> Self {
        Self {
            ptr,
            loop_,
            _data: data,
        }
    }
}

impl<'l> IsSource for SignalSource<'l> {
    fn as_ptr(&self) -> *mut spa_sys::spa_source {
        self.ptr.as_ptr()
    }
}

impl<'l> Drop for SignalSource<'l> {
    fn drop(&mut self) {
        unsafe { self.loop_.destroy_source(self) }
    }
}

/// A source that can be used to signal to a loop that an event has occurred.
///
/// This source can be obtained by calling [`add_event`](`LoopRef::add_event`) on a loop, registering a callback to it.
///
/// By calling [`signal`](`EventSource::signal`) on the `EventSource`, the loop is signaled that the event has occurred.
/// It will then call the callback at the next possible occasion.
pub struct EventSource<'l> {
    ptr: ptr::NonNull<spa_sys::spa_source>,
    loop_: &'l LoopRef,
    // Store data wrapper to prevent leak
    _data: Box<dyn Fn() + 'static>,
}

impl<'l> EventSource<'l> {
    pub(crate) fn new(
        ptr: ptr::NonNull<spa_sys::spa_source>,
        loop_: &'l LoopRef,
        data: Box<dyn Fn() + 'static>,
    ) -> Self {
        Self {
            ptr,
            loop_,
            _data: data,
        }
    }

    /// Signal the loop associated with this source that the event has occurred,
    /// to make the loop call the callback at the next possible occasion.
    pub fn signal(&self) -> SpaResult {
        let res = unsafe {
            let mut iface = self.loop_.as_raw().utils.as_ref().unwrap().iface;

            spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                signal_event,
                self.as_ptr()
            )
        };

        SpaResult::from_c(res)
    }
}

impl<'l> IsSource for EventSource<'l> {
    fn as_ptr(&self) -> *mut spa_sys::spa_source {
        self.ptr.as_ptr()
    }
}

impl<'l> Drop for EventSource<'l> {
    fn drop(&mut self) {
        unsafe { self.loop_.destroy_source(self) }
    }
}

/// A source that can be used to have a callback called on a timer.
///
/// This source can be obtained by calling [`add_timer`](`LoopRef::add_timer`) on a loop, registering a callback to it.
///
/// The timer starts out inactive.
/// You can arm or disarm the timer by calling [`update_timer`](`Self::update_timer`).
pub struct TimerSource<'l> {
    ptr: ptr::NonNull<spa_sys::spa_source>,
    loop_: &'l LoopRef,
    // Store data wrapper to prevent leak
    _data: Box<dyn Fn(u64) + 'static>,
}

impl<'l> TimerSource<'l> {
    pub(crate) fn new(
        ptr: ptr::NonNull<spa_sys::spa_source>,
        loop_: &'l LoopRef,
        data: Box<dyn Fn(u64) + 'static>,
    ) -> Self {
        Self {
            ptr,
            loop_,
            _data: data,
        }
    }

    /// Arm or disarm the timer.
    ///
    /// The timer will be called the next time after the provided `value` duration.
    /// After that, the timer will be repeatedly called again at the the specified `interval`.
    ///
    /// If `interval` is `None` or zero, the timer will only be called once. \
    /// If `value` is `None` or zero, the timer will be disabled.
    ///
    /// # Panics
    /// The provided durations seconds must fit in an i64. Otherwise, this function will panic.
    pub fn update_timer(&self, value: Option<Duration>, interval: Option<Duration>) -> SpaResult {
        fn duration_to_timespec(duration: Duration) -> spa_sys::timespec {
            spa_sys::timespec {
                tv_sec: duration.as_secs().try_into().expect("Duration too long"),
                // `Into` is only implemented on some platforms for these types,
                // so use a fallible conversion.
                // As there are a limited amount of nanoseconds in a second, this shouldn't fail
                #[allow(clippy::unnecessary_fallible_conversions)]
                tv_nsec: duration
                    .subsec_nanos()
                    .try_into()
                    .expect("Nanoseconds should fit into timespec"),
            }
        }

        let value = duration_to_timespec(value.unwrap_or_default());
        let interval = duration_to_timespec(interval.unwrap_or_default());

        let res = unsafe {
            let mut iface = self.loop_.as_raw().utils.as_ref().unwrap().iface;

            spa_interface_call_method!(
                &mut iface as *mut spa_sys::spa_interface,
                spa_sys::spa_loop_utils_methods,
                update_timer,
                self.as_ptr(),
                &value as *const _ as *mut _,
                &interval as *const _ as *mut _,
                false
            )
        };

        SpaResult::from_c(res)
    }
}

impl<'l> IsSource for TimerSource<'l> {
    fn as_ptr(&self) -> *mut spa_sys::spa_source {
        self.ptr.as_ptr()
    }
}

impl<'l> Drop for TimerSource<'l> {
    fn drop(&mut self) {
        unsafe { self.loop_.destroy_source(self) }
    }
}
