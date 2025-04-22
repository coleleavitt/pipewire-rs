use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use futures::channel::{oneshot, mpsc};
use futures::stream::{Stream, StreamExt};
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::stream::{Stream as PwStream, StreamFlags, StreamListener};
use crate::properties::Properties;
use crate::context::Context;
use crate::thread_loop::ThreadLoop;
use crate::error::Error;
use super::buffer::AsyncBuffer;
use super::utils::{TMR, TimeoutFuture, BoundedQueue};
use super::context::AsyncContextInner;

/// Async wrapper for PipeWire stream
pub struct AsyncStream {
    stream: PwStream,
    inner: Arc<AsyncContextInner>,
    state: Arc<Mutex<TMR<crate::stream::StreamState>>>,
    error: Arc<Mutex<Option<String>>>,
}

impl AsyncStream {
    /// Create a new async stream
    pub fn new(
        context: &mut super::context::AsyncContext,
        name: Option<&str>,
        props: Properties,
    ) -> Result<Self, Error> {
        let inner = context.inner.clone();

        inner.thread_loop.lock();

        // Create the stream
        let stream = PwStream::new(
            &inner.context,
            name.unwrap_or("async-stream"),
            props,
        )?;

        // Set up state tracking with radiation hardening
        let state = Arc::new(Mutex::new(TMR::new()));
        let state_clone = state.clone();

        // Track error messages
        let error = Arc::new(Mutex::new(None));
        let error_clone = error.clone();

        // Register for state change events
        let listener = stream.add_listener()
            .state_changed(move |old, new| {
                let mut state = state_clone.lock().unwrap();
                state.set(new);
            })
            .error(move |msg| {
                let mut error = error_clone.lock().unwrap();
                *error = Some(msg.to_string());
            })
            .register();

        inner.thread_loop.unlock();

        Ok(Self {
            stream,
            inner,
            state,
            error,
        })
    }

    /// Connect the stream asynchronously
    pub async fn connect(
        &self,
        direction: spa_sys::spa_direction,
        target_id: Option<u32>,
        flags: StreamFlags,
        params: &[spa_sys::spa_pod *],
    ) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();

        self.inner.thread_loop.lock();

        // Set up state change listener for connection
        let state_clone = self.state.clone();
        let tx_clone = tx.clone();

        let listener = self.stream.add_listener()
            .state_changed(move |old, new| {
                match new {
                    crate::stream::StreamState::Error => {
                        if !tx_clone.is_canceled() {
                            let _ = tx_clone.send(Err(Error::Other("Stream connection failed".into())));
                        }
                    },
                    crate::stream::StreamState::Paused |
                    crate::stream::StreamState::Streaming => {
                        if !tx_clone.is_canceled() {
                            let _ = tx_clone.send(Ok(()));
                        }
                    },
                    _ => {}
                }
            })
            .register();

        // Connect the stream
        self.stream.connect(direction, target_id, flags, params)?;

        self.inner.thread_loop.unlock();

        // Wait for the connection to complete with timeout
        let timeout_duration = std::time::Duration::from_secs(5);
        TimeoutFuture::new(rx, timeout_duration, 1000).await
            .map_err(|e| Error::Other(format!("Stream connection timeout: {}", e)))?
            .map_err(|e| Error::Other(format!("Stream connection error: {}", e)))
    }

    /// Process stream data asynchronously
    pub async fn process(&self) -> Result<Vec<AsyncBuffer>, Error> {
        // Dequeue buffers for processing
        let buffers = self.inner.thread_loop.sync_fn(|| {
            let mut result = Vec::new();
            while let Some(buffer) = self.stream.dequeue_buffer() {
                result.push(AsyncBuffer::new(buffer));
            }
            Ok(result)
        })?;

        Ok(buffers)
    }

    /// Get the current stream state
    pub fn state(&self) -> Result<crate::stream::StreamState, Error> {
        let state = self.state.lock().unwrap();
        state.get().ok_or_else(|| Error::Other("Stream state corruption detected".into()))
    }

    /// Get the underlying stream
    pub fn stream(&self) -> &PwStream {
        &self.stream
    }
}
