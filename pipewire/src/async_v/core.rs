use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use futures::channel::oneshot;
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::core;
use crate::thread_loop::ThreadLoop;
use crate::error::Error;
use super::registry::AsyncRegistry;
use super::utils::{TMR, TimeoutFuture};
use super::context::AsyncContextInner;

/// Async wrapper for PipeWire core
pub struct AsyncCore {
    core: core::Core,
    inner: Arc<AsyncContextInner>,
    seq: AtomicU32,
}

impl AsyncCore {
    /// Create a new async core
    pub(crate) fn new(core: core::Core, inner: Arc<AsyncContextInner>) -> Self {
        Self {
            core,
            inner,
            seq: AtomicU32::new(1),
        }
    }

    /// Get the registry asynchronously
    pub async fn get_registry(&self) -> Result<AsyncRegistry, Error> {
        let registry = self.core.get_registry()?;
        Ok(AsyncRegistry::new(registry, self.inner.clone(), self.clone()))
    }

    /// Synchronize with the PipeWire server asynchronously
    pub async fn sync(&self) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();

        // Generate a unique sequence number for this sync operation
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);

        self.inner.thread_loop.lock();

        // Set up listener for the done event
        let core_ref = &self.core;
        let listener = core_ref.add_listener_local()
            .done(move |id, seq_id| {
                if seq_id as u32 == seq {
                    if !tx.is_canceled() {
                        let _ = tx.send(Ok(()));
                    }
                }
            })
            .register();

        // Send the sync request
        self.core.sync(seq as i32)?;

        self.inner.thread_loop.unlock();

        // Wait for the sync to complete with timeout
        let timeout_duration = std::time::Duration::from_secs(5);
        TimeoutFuture::new(rx, timeout_duration, 1000).await
            .map_err(|e| Error::Other(format!("Sync timeout: {}", e)))?
            .map_err(|e| Error::Other(format!("Sync error: {}", e)))
    }

    /// Get the underlying core
    pub fn core(&self) -> &core::Core {
        &self.core
    }
}

impl Clone for AsyncCore {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            inner: self.inner.clone(),
            seq: AtomicU32::new(self.seq.load(Ordering::SeqCst)),
        }
    }
}
