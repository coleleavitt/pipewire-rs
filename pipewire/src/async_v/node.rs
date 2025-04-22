use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use futures::channel::{oneshot, mpsc};
use futures::stream::{Stream, StreamExt};
use futures::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::node::{Node};
use crate::proxy::ProxyT;
use crate::error::Error;
use super::utils::{TMR, TimeoutFuture, BoundedQueue};
use super::context::AsyncContextInner;

/// Async wrapper for PipeWire node
pub struct AsyncNode {
    node: Node,
    inner: Arc<AsyncContextInner>,
    info: Arc<Mutex<Option<crate::node::NodeInfo>>>,
}

impl AsyncNode {
    /// Create a new async node
    pub fn new(node: Node, inner: Arc<AsyncContextInner>) -> Self {
        let info = Arc::new(Mutex::new(None));

        // Set up info listener
        inner.thread_loop.lock();

        let info_clone = info.clone();
        let listener = node.add_listener_local()
            .info(move |info| {
                let mut info_lock = info_clone.lock().unwrap();
                *info_lock = Some(info.clone());
            })
            .register();

        inner.thread_loop.unlock();

        Self {
            node,
            inner,
            info,
        }
    }

    /// Get the node info asynchronously
    pub async fn get_info(&self) -> Result<crate::node::NodeInfo, Error> {
        // Check if we already have the info
        {
            let info = self.info.lock().unwrap();
            if let Some(ref info) = *info {
                return Ok(info.clone());
            }
        }

        // Wait for info to be received
        let (tx, rx) = oneshot::channel();

        self.inner.thread_loop.lock();

        let listener = self.node.add_listener_local()
            .info(move |info| {
                if !tx.is_canceled() {
                    let _ = tx.send(Ok(info.clone()));
                }
            })
            .register();

        // Trigger info update by subscribing to params
        self.node.subscribe_params_local(&[])?;

        self.inner.thread_loop.unlock();

        // Wait for the info with timeout
        let timeout_duration = std::time::Duration::from_secs(5);
        TimeoutFuture::new(rx, timeout_duration, 1000).await
            .map_err(|e| Error::Other(format!("Node info timeout: {}", e)))?
            .map_err(|e| Error::Other(format!("Node info error: {}", e)))
    }

    /// Set a node to run asynchronously (using PipeWire 1.2 async processing)
    pub async fn set_async(&self, enabled: bool) -> Result<(), Error> {
        self.inner.thread_loop.lock();

        // Set the async property on the node
        let props = crate::properties::Properties::new();
        props.set("node.async", if enabled { "true" } else { "false" });

        // Update the properties
        let result = self.node.update_properties(&props.to_dict());

        self.inner.thread_loop.unlock();

        result
    }

    /// Get the underlying node
    pub fn node(&self) -> &Node {
        &self.node
    }
}
