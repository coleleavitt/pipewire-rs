// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{
    mem,
    os,
    pin::Pin,
    ptr,
};

use super::{state::StreamState, stream::StreamRef};
use crate::error::Error;

type ParamChangedCB<D> = dyn FnMut(&StreamRef, &mut D, u32, Option<&spa::pod::Pod>);
type ProcessCB<D> = dyn FnMut(&StreamRef, &mut D);

/// Callbacks for stream events
#[allow(clippy::type_complexity)]
pub struct ListenerLocalCallbacks<D> {
    pub state_changed: Option<Box<dyn FnMut(&StreamRef, &mut D, StreamState, StreamState)>>,
    pub control_info:
        Option<Box<dyn FnMut(&StreamRef, &mut D, u32, *const pw_sys::pw_stream_control)>>,
    pub io_changed: Option<Box<dyn FnMut(&StreamRef, &mut D, u32, *mut os::raw::c_void, u32)>>,
    pub param_changed: Option<Box<ParamChangedCB<D>>>,
    pub add_buffer: Option<Box<dyn FnMut(&StreamRef, &mut D, *mut pw_sys::pw_buffer)>>,
    pub remove_buffer: Option<Box<dyn FnMut(&StreamRef, &mut D, *mut pw_sys::pw_buffer)>>,
    pub process: Option<Box<ProcessCB<D>>>,
    pub drained: Option<Box<dyn FnMut(&StreamRef, &mut D)>>,
    #[cfg(feature = "v0_3_39")]
    pub command: Option<Box<dyn FnMut(&StreamRef, &mut D, *const spa_sys::spa_command)>>,
    #[cfg(feature = "v0_3_40")]
    pub trigger_done: Option<Box<dyn FnMut(&StreamRef, &mut D)>>,
    pub user_data: D,
    pub(crate) stream: Option<ptr::NonNull<pw_sys::pw_stream>>,
}

unsafe fn unwrap_stream_ptr<'a>(stream: Option<ptr::NonNull<pw_sys::pw_stream>>) -> &'a StreamRef {
    stream
        .map(|ptr| ptr.cast::<StreamRef>().as_ref())
        .expect("stream cannot be null")
}

impl<D> ListenerLocalCallbacks<D> {
    pub(crate) fn with_user_data(user_data: D) -> Self {
        ListenerLocalCallbacks {
            process: Default::default(),
            stream: Default::default(),
            drained: Default::default(),
            add_buffer: Default::default(),
            control_info: Default::default(),
            io_changed: Default::default(),
            param_changed: Default::default(),
            remove_buffer: Default::default(),
            state_changed: Default::default(),
            #[cfg(feature = "v0_3_39")]
            command: Default::default(),
            #[cfg(feature = "v0_3_40")]
            trigger_done: Default::default(),
            user_data,
        }
    }

    pub(crate) fn into_raw(
        self,
    ) -> (
        Pin<Box<pw_sys::pw_stream_events>>,
        Box<ListenerLocalCallbacks<D>>,
    ) {
        let callbacks = Box::new(self);

        unsafe extern "C" fn on_state_changed<D>(
            data: *mut os::raw::c_void,
            old: pw_sys::pw_stream_state,
            new: pw_sys::pw_stream_state,
            error: *const os::raw::c_char,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.state_changed {
                    let stream = unwrap_stream_ptr(state.stream);
                    let old = StreamState::from_raw(old, error);
                    let new = StreamState::from_raw(new, error);
                    cb(stream, &mut state.user_data, old, new)
                };
            }
        }

        unsafe extern "C" fn on_control_info<D>(
            data: *mut os::raw::c_void,
            id: u32,
            control: *const pw_sys::pw_stream_control,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.control_info {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data, id, control);
                }
            }
        }

        unsafe extern "C" fn on_io_changed<D>(
            data: *mut os::raw::c_void,
            id: u32,
            area: *mut os::raw::c_void,
            size: u32,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.io_changed {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data, id, area, size);
                }
            }
        }

        unsafe extern "C" fn on_param_changed<D>(
            data: *mut os::raw::c_void,
            id: u32,
            param: *const spa_sys::spa_pod,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.param_changed {
                    let stream = unwrap_stream_ptr(state.stream);
                    let param = if !param.is_null() {
                        Some(spa::pod::Pod::from_raw(param))
                    } else {
                        None
                    };

                    cb(stream, &mut state.user_data, id, param);
                }
            }
        }

        unsafe extern "C" fn on_add_buffer<D>(
            data: *mut ::std::os::raw::c_void,
            buffer: *mut pw_sys::pw_buffer,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.add_buffer {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data, buffer);
                }
            }
        }

        unsafe extern "C" fn on_remove_buffer<D>(
            data: *mut ::std::os::raw::c_void,
            buffer: *mut pw_sys::pw_buffer,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.remove_buffer {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data, buffer);
                }
            }
        }

        unsafe extern "C" fn on_process<D>(data: *mut ::std::os::raw::c_void) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.process {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data);
                }
            }
        }

        unsafe extern "C" fn on_drained<D>(data: *mut ::std::os::raw::c_void) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.drained {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data);
                }
            }
        }

        #[cfg(feature = "v0_3_39")]
        unsafe extern "C" fn on_command<D>(
            data: *mut ::std::os::raw::c_void,
            command: *const spa_sys::spa_command,
        ) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.command {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data, command);
                }
            }
        }

        #[cfg(feature = "v0_3_40")]
        unsafe extern "C" fn on_trigger_done<D>(data: *mut ::std::os::raw::c_void) {
            if let Some(state) = (data as *mut ListenerLocalCallbacks<D>).as_mut() {
                if let Some(cb) = &mut state.trigger_done {
                    let stream = unwrap_stream_ptr(state.stream);
                    cb(stream, &mut state.user_data);
                }
            }
        }

        let events = unsafe {
            let mut events: Pin<Box<pw_sys::pw_stream_events>> = Box::pin(mem::zeroed());
            events.version = pw_sys::PW_VERSION_STREAM_EVENTS;

            if callbacks.state_changed.is_some() {
                events.state_changed = Some(on_state_changed::<D>);
            }
            if callbacks.control_info.is_some() {
                events.control_info = Some(on_control_info::<D>);
            }
            if callbacks.io_changed.is_some() {
                events.io_changed = Some(on_io_changed::<D>);
            }
            if callbacks.param_changed.is_some() {
                events.param_changed = Some(on_param_changed::<D>);
            }
            if callbacks.add_buffer.is_some() {
                events.add_buffer = Some(on_add_buffer::<D>);
            }
            if callbacks.remove_buffer.is_some() {
                events.remove_buffer = Some(on_remove_buffer::<D>);
            }
            if callbacks.process.is_some() {
                events.process = Some(on_process::<D>);
            }
            if callbacks.drained.is_some() {
                events.drained = Some(on_drained::<D>);
            }
            #[cfg(feature = "v0_3_39")]
            if callbacks.command.is_some() {
                events.command = Some(on_command::<D>);
            }
            #[cfg(feature = "v0_3_40")]
            if callbacks.trigger_done.is_some() {
                events.trigger_done = Some(on_trigger_done::<D>);
            }

            events
        };

        (events, callbacks)
    }
}

/// Builder for stream listeners
#[must_use]
pub struct ListenerLocalBuilder<'a, D> {
    pub(crate) stream: &'a StreamRef,
    pub(crate) callbacks: ListenerLocalCallbacks<D>,
}

impl<'a, D> ListenerLocalBuilder<'a, D> {
    /// Set the callback for the `state_changed` event.
    pub fn state_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, StreamState, StreamState) + 'static,
    {
        self.callbacks.state_changed = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `control_info` event.
    pub fn control_info<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, u32, *const pw_sys::pw_stream_control) + 'static,
    {
        self.callbacks.control_info = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `io_changed` event.
    pub fn io_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, u32, *mut os::raw::c_void, u32) + 'static,
    {
        self.callbacks.io_changed = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `param_changed` event.
    pub fn param_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, u32, Option<&spa::pod::Pod>) + 'static,
    {
        self.callbacks.param_changed = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `add_buffer` event.
    pub fn add_buffer<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, *mut pw_sys::pw_buffer) + 'static,
    {
        self.callbacks.add_buffer = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `remove_buffer` event.
    pub fn remove_buffer<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D, *mut pw_sys::pw_buffer) + 'static,
    {
        self.callbacks.remove_buffer = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `process` event.
    pub fn process<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D) + 'static,
    {
        self.callbacks.process = Some(Box::new(callback));
        self
    }

    /// Set the callback for the `drained` event.
    pub fn drained<F>(mut self, callback: F) -> Self
    where
        F: FnMut(&StreamRef, &mut D) + 'static,
    {
        self.callbacks.drained = Some(Box::new(callback));
        self
    }

    /// Register the Callbacks
    ///
    /// Stop building the listener and register it on the stream. Returns a
    /// `StreamListener` handle that will un-register the listener on drop.
    pub fn register(self) -> Result<StreamListener<D>, Error> {
        let (events, data) = self.callbacks.into_raw();
        let (listener, data) = unsafe {
            let listener: Box<spa_sys::spa_hook> = Box::new(mem::zeroed());
            let raw_listener = Box::into_raw(listener);
            let raw_data = Box::into_raw(data);
            pw_sys::pw_stream_add_listener(
                self.stream.as_raw_ptr(),
                raw_listener,
                events.as_ref().get_ref(),
                raw_data as *mut _,
            );
            (Box::from_raw(raw_listener), Box::from_raw(raw_data))
        };
        Ok(StreamListener {
            listener,
            _events: events,
            _data: data,
        })
    }
}

/// Handle for a registered stream listener
pub struct StreamListener<D> {
    listener: Box<spa_sys::spa_hook>,
    // Need to stay allocated while the listener is registered
    _events: Pin<Box<pw_sys::pw_stream_events>>,
    _data: Box<ListenerLocalCallbacks<D>>,
}

impl<D> StreamListener<D> {
    /// Stop the listener from receiving any events
    ///
    /// Removes the listener registration and cleans up allocated resources.
    pub fn unregister(self) {
        // do nothing, drop will clean up.
    }
}

impl<D> std::ops::Drop for StreamListener<D> {
    fn drop(&mut self) {
        spa::utils::hook::remove(*self.listener);
    }
}
