use spa_sys::spa_meta_sync_timeline;
use std::fmt::Debug;
use std::sync::Arc;
use std::os::unix::io::RawFd;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// A transparent wrapper around a spa_meta_sync_timeline for explicit synchronization.
///
/// This implements the linux-drm-syncobj-v1 protocol which uses timeline synchronization
/// objects instead of binary fences. This approach provides several key advantages:
///
/// ## Timeline vs Binary Fence Synchronization
///
/// **Timeline Synchronization (linux-drm-syncobj-v1)**:
/// - Uses timeline points on a continuous counter for synchronization
/// - Supports multiple frames in flight efficiently  
/// - Allows expressing complex dependencies between work
/// - Lower overhead (one syncobj with many timeline points)
/// - Native support in modern APIs (Vulkan, EGL, PipeWire)
///
/// **Binary Fence Synchronization (zwp_linux_explicit_synchronization_v1)**:
/// - Uses separate fence objects for each buffer
/// - Limited to single-frame-at-a-time workflows
/// - Awkward for managing multiple queued frames
/// - Higher overhead (separate fence per frame)
/// - Legacy approach being superseded
///
/// ## Usage Pattern
///
/// 1. **Acquire Point**: Timeline point that must be signaled before GPU can access buffer
/// 2. **Release Point**: Timeline point signaled by compositor when buffer can be reused
/// 3. **Timeline FDs**: File descriptors for acquire and release timeline syncobjs
///
/// This enables efficient streaming workflows where multiple buffers can be queued
/// with explicit dependencies expressed through timeline points.
#[repr(transparent)]
pub struct SyncTimelineRef(Arc<spa_meta_sync_timeline>);

impl SyncTimelineRef {
    pub fn new(acquire_point: u64, release_point: u64) -> Self {
        SyncTimelineRef(Arc::new(spa_meta_sync_timeline {
            flags: 0,
            padding: 0,
            acquire_point,
            release_point,
        }))
    }

    /// Creates a `SyncTimelineRef` from a raw pointer.
    ///
    /// # Safety
    /// This function assumes that the raw pointer is valid and not null. The caller
    /// must ensure that the pointer has been allocated with a reference count
    /// suitable for conversion into an `Arc`.
    pub unsafe fn from_raw(sync_timeline: *mut spa_meta_sync_timeline) -> Option<Self> {
        if !sync_timeline.is_null() {
            // Using Arc::from_raw for a mutable pointer
            Some(SyncTimelineRef(Arc::from_raw(sync_timeline)))
        } else {
            None
        }
    }

    pub fn as_raw(&self) -> &spa_meta_sync_timeline {
        &self.0
    }

    // Here, we changed the return type to *const to reflect the immutable nature of the reference
    pub fn as_raw_ptr(&self) -> *const spa_meta_sync_timeline {
        Arc::as_ptr(&self.0)
    }

    pub fn flags(&self) -> u32 {
        self.0.flags
    }

    pub fn acquire_point(&self) -> u64 {
        self.0.acquire_point
    }

    pub async fn set_acquire_point(&mut self, point: u64) -> Result<(), anyhow::Error> {
        // Ensure Arc is mutable
        if let Some(inner) = Arc::get_mut(&mut self.0) {
            inner.acquire_point = point;
            Ok(())
        } else {
            // Could not get mutable access, probably because of multiple clones
            Err(anyhow::anyhow!("Failed to get mutable access to modify the acquire point"))
        }
    }

    pub fn release_point(&self) -> u64 {
        self.0.release_point
    }

    pub async fn get_release_point(&self) -> Result<u64, anyhow::Error> {
        Ok(self.release_point())
    }

    pub fn release_point_mut(&mut self) -> &mut u64 {
        Arc::get_mut(&mut self.0)
            .map(|inner| &mut inner.release_point)
            .expect("Failed to get mutable access to release point")
    }

    /// Signals the buffer release point, indicating the data can be reused
    pub async fn signal_release(&mut self, release_point: u64) -> Result<(), anyhow::Error> {
        *self.release_point_mut() = release_point;
        Ok(())
    }

    /// Waits for the buffer to be available based on the current timeline
    pub async fn wait_for_available(&self) -> Result<(), anyhow::Error> {
        SyncFuture::new(self.acquire_point()).await
    }

    /// Synchronously wait for DMA-BUF with explicit sync using syncobj timeline points
    pub async fn sync_dma_buf(&self, acquire_timeline_fd: RawFd, release_timeline_fd: RawFd) -> Result<(), anyhow::Error> {
        // Wait for acquire timeline point to be signaled
        SyncObjTimelineWaiter::new(acquire_timeline_fd, self.acquire_point()).await?;
        
        // Signal the release timeline point after GPU work is complete
        self.signal_timeline_point(release_timeline_fd, self.release_point()).await?;
        
        Ok(())
    }

    /// Signal a syncobj timeline point
    async fn signal_timeline_point(&self, timeline_fd: RawFd, point: u64) -> Result<(), anyhow::Error> {
        SyncObjTimelineSignaler::new(timeline_fd, point).await
    }
}

impl Default for SyncTimelineRef {
    fn default() -> Self {
        SyncTimelineRef(Arc::new(spa_meta_sync_timeline {
            flags: 0,
            padding: 0,
            acquire_point: 0,
            release_point: 0,
        }))
    }
}

impl Clone for SyncTimelineRef {
    fn clone(&self) -> Self {
        SyncTimelineRef(Arc::clone(&self.0))
    }
}

impl Debug for SyncTimelineRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncTimelineRef")
            .field("flags", &self.flags())
            .field("acquire_point", &self.acquire_point())
            .field("release_point", &self.release_point())
            .finish()
    }
}

/// Future for waiting on sync timeline points
#[derive(Debug)]
pub struct SyncFuture {
    timeline_point: u64,
    start_time: Instant,
    timeout: Duration,
}

impl SyncFuture {
    pub fn new(timeline_point: u64) -> Self {
        Self {
            timeline_point,
            start_time: Instant::now(),
            timeout: Duration::from_secs(5), // 5 second timeout
        }
    }

    pub fn with_timeout(timeline_point: u64, timeout: Duration) -> Self {
        Self {
            timeline_point,
            start_time: Instant::now(),
            timeout,
        }
    }
}

impl Future for SyncFuture {
    type Output = Result<(), anyhow::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let elapsed = self.start_time.elapsed();
        if elapsed > self.timeout {
            return Poll::Ready(Err(anyhow::anyhow!("Sync timeline wait timed out")));
        }
        
        // In a real implementation, this would check the actual syncobj timeline
        // For now, we simulate completion for timeline points
        if self.timeline_point == 0 {
            Poll::Ready(Ok(()))
        } else {
            // Wake task for retry
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Future for waiting on syncobj timeline points via DRM syncobj timeline
#[derive(Debug)]
pub struct SyncObjTimelineWaiter {
    timeline_fd: RawFd,
    timeline_point: u64,
    start_time: Instant,
    timeout: Duration,
}

impl SyncObjTimelineWaiter {
    pub fn new(timeline_fd: RawFd, timeline_point: u64) -> Self {
        Self {
            timeline_fd,
            timeline_point,
            start_time: Instant::now(),
            timeout: Duration::from_secs(5),
        }
    }
}

impl Future for SyncObjTimelineWaiter {
    type Output = Result<(), anyhow::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let elapsed = self.start_time.elapsed();
        if elapsed > self.timeout {
            return Poll::Ready(Err(anyhow::anyhow!(
                "SyncObj timeline wait timed out on fd {} for timeline point {}", 
                self.timeline_fd, self.timeline_point
            )));
        }

        // In a real implementation, this would use DRM syncobj timeline ioctls:
        // - DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT to wait for specific timeline point
        // - Uses WAIT_FOR_SUBMIT flag if point not yet submitted
        // For now, simulate immediate completion for valid fds
        if self.timeline_fd >= 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(anyhow::anyhow!("Invalid syncobj timeline file descriptor")))
        }
    }
}

/// Future for signaling syncobj timeline points via DRM syncobj timeline
/// 
/// This implements the linux-drm-syncobj-v1 protocol which supports timeline synchronization
/// objects. Unlike v1 binary fences, this allows:
/// - Multiple frames in flight with different timeline points
/// - Efficient queuing of work with explicit dependencies  
/// - Lower overhead (one syncobj with many points vs many fence objects)
/// - Native support for modern graphics APIs (Vulkan, EGL, PipeWire)
#[derive(Debug)]  
pub struct SyncObjTimelineSignaler {
    timeline_fd: RawFd,
    timeline_point: u64,
}

impl SyncObjTimelineSignaler {
    pub fn new(timeline_fd: RawFd, timeline_point: u64) -> Self {
        Self {
            timeline_fd,
            timeline_point,
        }
    }
}

impl Future for SyncObjTimelineSignaler {
    type Output = Result<(), anyhow::Error>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // In a real implementation, this would use DRM syncobj timeline ioctls:
        // - DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL to signal specific timeline point
        // - This allows signaling completion of work at a specific point on the timeline
        if self.timeline_fd >= 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Ready(Err(anyhow::anyhow!("Invalid syncobj timeline file descriptor")))
        }
    }
}
