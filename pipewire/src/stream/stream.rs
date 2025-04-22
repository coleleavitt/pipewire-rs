// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use crate::buffer::Buffer;
use crate::{
    core::Core,
    error::Error,
    properties::{Properties, PropertiesRef},
};
use spa::utils::result::SpaResult;
use std::{
    ffi::{self, CStr, CString},
    fmt::Debug,
    pin::Pin,
    ptr,
};

use super::flags::StreamFlags;
use super::state::StreamState;
use super::listener::{ListenerLocalBuilder, ListenerLocalCallbacks};

/// A wrapper around the pipewire stream interface. Streams are a higher
/// level abstraction around nodes in the graph. A stream can be used to send or
/// receive frames of audio or video data by connecting it to another node.
pub struct Stream {
    ptr: ptr::NonNull<pw_sys::pw_stream>,
    // objects that need to stay alive while the Stream is
    _core: Core,
}

impl Stream {
    /// Create a [`Stream`]
    ///
    /// Initialises a new stream with the given `name` and `properties`.
    pub fn new(core: &Core, name: &str, properties: Properties) -> Result<Self, Error> {
        let name = CString::new(name).expect("Invalid byte in stream name");
        let c_str = name.as_c_str();
        Stream::new_cstr(core, c_str, properties)
    }

    /// Initialises a new stream with the given `name` as Cstr and `properties`.
    pub fn new_cstr(core: &Core, name: &CStr, properties: Properties) -> Result<Self, Error> {
        let stream = unsafe {
            pw_sys::pw_stream_new(core.as_raw_ptr(), name.as_ptr(), properties.into_raw())
        };
        let stream = ptr::NonNull::new(stream).ok_or(Error::CreationFailed)?;

        Ok(Stream {
            ptr: stream,
            _core: core.clone(),
        })
    }

    pub fn into_raw(self) -> *mut pw_sys::pw_stream {
        let mut this = std::mem::ManuallyDrop::new(self);

        // FIXME: self needs to be wrapped in ManuallyDrop so the raw stream
        //        isn't destroyed. However, the core should still be dropped.
        //        Is there a cleaner and safer way to drop the core than like this?
        unsafe {
            ptr::drop_in_place(ptr::addr_of_mut!(this._core));
        }

        this.ptr.as_ptr()
    }
}

impl std::ops::Deref for Stream {
    type Target = StreamRef;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.cast().as_ref() }
    }
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("name", &self.name())
            .field("state", &self.state())
            .field("node-id", &self.node_id())
            .field("properties", &self.properties())
            .finish()
    }
}

impl std::ops::Drop for Stream {
    fn drop(&mut self) {
        unsafe { pw_sys::pw_stream_destroy(self.as_raw_ptr()) }
    }
}

/// Reference to a PipeWire stream
#[repr(transparent)]
pub struct StreamRef(pw_sys::pw_stream);

impl StreamRef {
    /// Get raw pointer to the stream
    pub fn as_raw(&self) -> &pw_sys::pw_stream {
        &self.0
    }

    /// Get mutable raw pointer to the stream
    pub fn as_raw_ptr(&self) -> *mut pw_sys::pw_stream {
        ptr::addr_of!(self.0).cast_mut()
    }

    /// Add a local listener builder with custom user data
    #[must_use = "Fluent builder API"]
    pub fn add_local_listener_with_user_data<D>(
        &self,
        user_data: D,
    ) -> ListenerLocalBuilder<'_, D> {
        let mut callbacks = ListenerLocalCallbacks::with_user_data(user_data);
        callbacks.stream =
            Some(ptr::NonNull::new(self.as_raw_ptr()).expect("Pointer should be nonnull"));
        ListenerLocalBuilder {
            stream: self,
            callbacks,
        }
    }

    /// Add a local listener builder with default user data
    #[must_use = "Fluent builder API"]
    pub fn add_local_listener<D: Default>(&self) -> ListenerLocalBuilder<'_, D> {
        self.add_local_listener_with_user_data(Default::default())
    }

    /// Connect the stream
    ///
    /// Tries to connect to the node `id` in the given `direction`. If no node
    /// is provided then any suitable node will be used.
    pub fn connect(
        &self,
        direction: spa::utils::Direction,
        id: Option<u32>,
        flags: StreamFlags,
        params: &mut [&spa::pod::Pod],
    ) -> Result<(), Error> {
        let r = unsafe {
            pw_sys::pw_stream_connect(
                self.as_raw_ptr(),
                direction.as_raw(),
                id.unwrap_or(crate::constants::ID_ANY),
                flags.bits(),
                // We cast from *mut [&spa::pod::Pod] to *mut [*const spa_sys::spa_pod] here,
                // which is valid because spa::pod::Pod is a transparent wrapper around spa_sys::spa_pod
                params.as_mut_ptr().cast(),
                params.len() as u32,
            )
        };

        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    /// Update Parameters
    ///
    /// Call from the `param_changed` callback to negotiate a new set of
    /// parameters for the stream.
    pub fn update_params(&self, params: &mut [&spa::pod::Pod]) -> Result<(), Error> {
        let r = unsafe {
            pw_sys::pw_stream_update_params(
                self.as_raw_ptr(),
                params.as_mut_ptr().cast(),
                params.len() as u32,
            )
        };

        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    /// Activate or deactivate the stream
    pub fn set_active(&self, active: bool) -> Result<(), Error> {
        let r = unsafe { pw_sys::pw_stream_set_active(self.as_raw_ptr(), active) };

        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    /// Take a Buffer from the Stream
    ///
    /// Removes a buffer from the stream. If this is an input stream the buffer
    /// will contain data ready to process. If this is an output stream it can
    /// be filled.
    ///
    /// # Safety
    ///
    /// The pointer returned could be NULL if no buffer is available. The buffer
    /// should be returned to the stream once processing is complete.
    pub unsafe fn dequeue_raw_buffer(&self) -> *mut pw_sys::pw_buffer {
        pw_sys::pw_stream_dequeue_buffer(self.as_raw_ptr())
    }

    /// Dequeue a buffer from the stream
    pub fn dequeue_buffer(&self) -> Option<Buffer> {
        unsafe { Buffer::from_raw(self.dequeue_raw_buffer(), self) }
    }

    /// Async wrapper for dequeue_buffer for compatibility
    pub async fn dequeue_buffer_async(&self) -> Result<Option<Buffer>, Error> {
        Ok(self.dequeue_buffer())
    }

    /// Return a Buffer to the Stream
    ///
    /// Give back a buffer once processing is complete. Use this to queue up a
    /// frame for an output stream, or return the buffer to the pool ready to
    /// receive new data for an input stream.
    ///
    /// # Safety
    ///
    /// The buffer pointer should be one obtained from this stream instance by
    /// a call to [StreamRef::dequeue_raw_buffer()].
    pub unsafe fn queue_raw_buffer(&self, buffer: *mut pw_sys::pw_buffer) {
        pw_sys::pw_stream_queue_buffer(self.as_raw_ptr(), buffer);
    }

    /// Disconnect the stream
    pub fn disconnect(&self) -> Result<(), Error> {
        let r = unsafe { pw_sys::pw_stream_disconnect(self.as_raw_ptr()) };

        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    /// Set the stream in error state
    ///
    /// # Panics
    /// Will panic if `error` contains a 0 byte.
    pub fn set_error(&mut self, res: i32, error: &str) {
        let error = CString::new(error).expect("failed to convert error to CString");
        let error_cstr = error.as_c_str();
        StreamRef::set_error_cstr(self, res, error_cstr)
    }

    /// Set the stream in error state with CStr
    pub fn set_error_cstr(&mut self, res: i32, error: &CStr) {
        unsafe {
            pw_sys::pw_stream_set_error(self.as_raw_ptr(), res, error.as_ptr());
        }
    }

    /// Flush the stream. When `drain` is `true`, the `drained` callback will
    /// be called when all data is played or recorded.
    pub fn flush(&self, drain: bool) -> Result<(), Error> {
        let r = unsafe { pw_sys::pw_stream_flush(self.as_raw_ptr(), drain) };

        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    /// Set control values on the stream
    pub fn set_control(&self, id: u32, values: &[f32]) -> Result<(), Error> {
        let r = unsafe {
            pw_sys::pw_stream_set_control(
                self.as_raw_ptr(),
                id,
                values.len() as u32,
                values.as_ptr() as *mut f32,
            )
        };
        SpaResult::from_c(r).into_sync_result()?;
        Ok(())
    }

    // Getter methods

    /// Get the name of the stream
    pub fn name(&self) -> String {
        let name = unsafe {
            let name = pw_sys::pw_stream_get_name(self.as_raw_ptr());
            CStr::from_ptr(name)
        };

        name.to_string_lossy().to_string()
    }

    /// Get the current state of the stream
    pub fn state(&self) -> StreamState {
        let mut error: *const std::os::raw::c_char = ptr::null();
        let state = unsafe {
            pw_sys::pw_stream_get_state(self.as_raw_ptr(), (&mut error) as *mut *const _)
        };
        StreamState::from_raw(state, error)
    }

    /// Get the properties of the stream
    pub fn properties(&self) -> &PropertiesRef {
        unsafe {
            let props = pw_sys::pw_stream_get_properties(self.as_raw_ptr());
            let props = ptr::NonNull::new(props.cast_mut()).expect("stream properties is NULL");
            props.cast().as_ref()
        }
    }

    /// Get the node ID of the stream
    pub fn node_id(&self) -> u32 {
        unsafe { pw_sys::pw_stream_get_node_id(self.as_raw_ptr()) }
    }

    #[cfg(feature = "v0_3_34")]
    pub fn is_driving(&self) -> bool {
        unsafe { pw_sys::pw_stream_is_driving(self.as_raw_ptr()) }
    }

    #[cfg(feature = "v0_3_34")]
    pub fn trigger_process(&self) -> Result<(), Error> {
        let r = unsafe { pw_sys::pw_stream_trigger_process(self.as_raw_ptr()) };

        SpaResult::from_c(r).into_result()?;
        Ok(())
    }
}
