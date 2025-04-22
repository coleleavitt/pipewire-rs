use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use futures::channel::{oneshot, mpsc};
use futures::stream::{Stream, StreamExt};
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::registry::{Registry, GlobalObject};
use crate::proxy::Proxy;
use crate::error::Error;
use super::core::AsyncCore;
use super::utils::{TMR, TimeoutFuture, BoundedQueue};
use super::context::AsyncContextInner;

/// Async wrapper for PipeWire registry
pub struct AsyncRegistry {
    registry: Registry,
    inner: Arc<AsyncContextInner>,
    core: AsyncCore,
}

impl AsyncRegistry {
    /// Create a new async registry
    pub(crate) fn new(registry: Registry, inner: Arc<AsyncContextInner>, core: AsyncCore) -> Self {
        Self {
            registry,
            inner,
            core,
        }
    }

    /// List all global objects asynchronously
    pub async fn list_objects<T: crate::proxy::ProxyT>(&self) -> Result<Vec<T>, Error> {
        // Create a channel for collecting objects
        let (tx, mut rx) = mpsc::channel(16);

        self.inner.thread_loop.lock();

        // Track if we've completed the object listing
        let done = Arc::new(AtomicBool::new(false));
        let done_clone = done.clone();

        // Register for global events
        let listener = self.registry.add_listener_local()
            .global(move |global| {
                if global.type_ == T::type_() {
                    if let Ok(proxy) = Proxy::from_global(global) {
                        if let Ok(obj) = T::from_proxy(proxy) {
                            let _ = tx.clone().try_send(obj);
                        }
                    }
                }
            })
            .global_remove(move |id| {
                // Handle object removal if needed
            })
            .register();

        // Set up a listener for the sync done event
        let tx_done = tx.clone();
        let core_ref = &self.core.core();
        let listener_done = core_ref.add_listener_local()
            .done(move |id, seq| {
                // Signal completion
                done_clone.store(true, Ordering::SeqCst);
                let _ = tx_done.clone().close();
            })
            .register();

        // Request registry sync to trigger callbacks
        let seq = core_ref.sync(0)?;

        self.inner.thread_loop.unlock();

        // Collect results with radiation-hardened error handling
        let mut objects = Vec::new();

        // Create a timeout for the overall operation
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);

        while let Some(obj) = futures::select! {
            obj = rx.next() => obj,
            _ = futures::future::ready(()) => {
                if done.load(Ordering::SeqCst) {
                    None
                } else if start_time.elapsed() > timeout {
                    return Err(Error::Other("Timeout listing objects".into()));
                } else {
                    continue;
                }
            }
        } {
            // Bounded collection size for predictable memory usage
            if objects.len() < 1024 {
                objects.push(obj);
            } else {
                return Err(Error::Other("Too many objects".into()));
            }
        }

        Ok(objects)
    }

    /// Get the underlying registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}
