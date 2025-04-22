use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use futures::channel::oneshot;
use futures::Future;
use futures::task::AtomicWaker;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::lock::Mutex;
use crate::context;
use crate::core;
use crate::thread_loop::ThreadLoop;
use crate::properties::Properties;
use crate::error::Error;
use super::core::AsyncCore;
use super::utils::{TMR, TimeoutFuture};

/// Async wrapper for PipeWire context
pub struct AsyncContext {
    inner: Arc<AsyncContextInner>,
}

struct AsyncContextInner {
    context: context::Context,
    thread_loop: ThreadLoop,
    running: AtomicBool,
    // Triple redundancy for radiation hardening
    cores: Mutex<TMR<core::Core>>,
}

impl AsyncContext {
    /// Create a new async context with radiation-hardened patterns
    pub fn new() -> Result<Self, Error> {
        // Create a thread loop with bounded execution guarantees
        let thread_loop = ThreadLoop::new_full(
            None,
            "pw-async-loop",
            &Properties::new_dict(&[("loop.cancel", "true")])?,
        )?;

        // Create the context on the thread loop
        let context = context::Context::new(&thread_loop.loop_())?;

        let inner = Arc::new(AsyncContextInner {
            context,
            thread_loop,
            running: AtomicBool::new(false),
            cores: Mutex::new(TMR::new()),
        });

        Ok(Self { inner })
    }

    /// Start the context loop
    pub fn start(&self) -> Result<(), Error> {
        if self.inner.running.swap(true, Ordering::SeqCst) {
            return Ok(());  // Already running
        }

        self.inner.thread_loop.start()?;
        Ok(())
    }

    /// Stop the context loop
    pub fn stop(&self) -> Result<(), Error> {
        if !self.inner.running.swap(false, Ordering::SeqCst) {
            return Ok(());  // Already stopped
        }

        self.inner.thread_loop.stop()?;
        Ok(())
    }

    /// Connect to PipeWire asynchronously
    pub async fn connect(&self) -> Result<AsyncCore, Error> {
        let (tx, rx) = oneshot::channel();

        // Execute the connect operation on the thread loop
        self.inner.thread_loop.lock();

        // Create a core connection
        let core = self.inner.context.connect(None)?;

        // Set up listeners to detect when the connection is ready
        let listener = core.add_listener_local()
            .info(move |info| {
                // Connection is ready
                if !tx.is_canceled() {
                    let _ = tx.send(Ok(()));
                }
            })
            .error(move |id, seq, res, message| {
                // Connection failed
                if !tx.is_canceled() {
                    let _ = tx.send(Err(Error::Other(message.to_string())));
                }
            })
            .register();

        // Store the core with triple redundancy
        {
            let mut cores = self.inner.cores.lock().unwrap();
            cores.set(core.clone());
        }

        self.inner.thread_loop.unlock();

        // Wait for the connection to complete with timeout
        let timeout_duration = std::time::Duration::from_secs(5);
        let result = TimeoutFuture::new(rx, timeout_duration, 1000).await
            .map_err(|e| Error::Other(format!("Connection timeout: {}", e)))?
            .map_err(|e| Error::Other(format!("Connection error: {}", e)))?;

        // Create an async core wrapper
        Ok(AsyncCore::new(core, self.inner.clone()))
    }
}

impl Drop for AsyncContext {
    fn drop(&mut self) {
        // Ensure the thread loop is stopped when the context is dropped
        if self.inner.running.load(Ordering::SeqCst) {
            let _ = self.inner.thread_loop.stop();
        }
    }
}
