// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::fmt::Debug;
use std::os::fd::RawFd;
use std::sync::atomic::Ordering;

/// Error types specific to timeline synchronization
#[derive(Debug, thiserror::Error)]
pub enum TimelineError {
    #[error("Buffer is not available yet")]
    NotAvailable,
    #[error("Failed to synchronize: {0}")]
    SyncFailed(#[from] std::io::Error),
    #[error("Invalid operation")]
    InvalidOperation,
}

/// A transparent wrapper around a spa_meta_sync_timeline for explicit synchronization.
///
/// This references a timeline metadata in a buffer, which is used for explicit
/// synchronization between producers and consumers of shared buffers.
#[repr(transparent)]
pub struct SyncTimelineRef<'a>(&'a mut spa_sys::spa_meta_sync_timeline);

impl<'a> SyncTimelineRef<'a> {
    /// Creates a `SyncTimelineRef` from a raw pointer.
    ///
    /// # Safety
    /// The pointer must be valid and pointing to a properly allocated timeline metadata
    /// that lives at least as long as the returned reference.
    pub unsafe fn from_raw(ptr: *mut spa_sys::spa_meta_sync_timeline) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(&mut *ptr))
        }
    }

    /// Get a reference to the underlying spa_meta_sync_timeline
    pub fn as_raw(&self) -> &spa_sys::spa_meta_sync_timeline {
        self.0
    }

    /// Get a mutable reference to the underlying spa_meta_sync_timeline
    pub fn as_raw_mut(&mut self) -> &mut spa_sys::spa_meta_sync_timeline {
        self.0
    }

    /// Get a raw pointer to the underlying timeline
    pub fn as_ptr(&self) -> *const spa_sys::spa_meta_sync_timeline {
        self.0
    }

    /// Get a mutable raw pointer to the underlying timeline
    pub fn as_mut_ptr(&mut self) -> *mut spa_sys::spa_meta_sync_timeline {
        self.0 as *mut _
    }

    /// Get the flags field of the timeline
    pub fn flags(&self) -> u32 {
        std::sync::atomic::fence(Ordering::Acquire);
        self.0.flags
    }

    /// Set the flags field of the timeline
    pub fn set_flags(&mut self, flags: u32) {
        self.0.flags = flags;
        std::sync::atomic::fence(Ordering::Release);
    }

    /// Get the acquire point of the timeline
    ///
    /// The acquire point indicates when the buffer data is ready to be processed.
    /// Consumers should wait until the current timeline point reaches or exceeds
    /// this value before accessing the buffer.
    pub fn acquire_point(&self) -> u64 {
        std::sync::atomic::fence(Ordering::Acquire);
        self.0.acquire_point
    }

    /// Set the acquire point of the timeline
    ///
    /// The acquire point indicates when the buffer data is ready to be processed.
    /// This is typically called by producers when they have finished writing to
    /// the buffer.
    pub fn set_acquire_point(&mut self, point: u64) {
        self.0.acquire_point = point;
        std::sync::atomic::fence(Ordering::Release);
    }

    /// Get the release point of the timeline
    ///
    /// The release point indicates when the buffer can be reused by the producer.
    /// Producers should wait until the current timeline point reaches or exceeds
    /// this value before modifying the buffer.
    pub fn release_point(&self) -> u64 {
        std::sync::atomic::fence(Ordering::Acquire);
        self.0.release_point
    }

    /// Set the release point of the timeline
    ///
    /// This signals that the buffer can be reused once the timeline reaches this point.
    /// Typically called by consumers when they have finished reading from the buffer.
    pub fn set_release_point(&mut self, point: u64) {
        self.0.release_point = point;
        std::sync::atomic::fence(Ordering::Release);
    }

    /// Check if the buffer is available for processing
    ///
    /// This returns true if the acquire point is greater than or equal to the release point,
    /// indicating that the buffer is ready for processing.
    pub fn is_available(&self) -> bool {
        self.acquire_point() >= self.release_point()
    }

    /// Wait for the buffer to be available
    ///
    /// Returns Ok if the buffer is available for processing, or an error if it's not.
    pub fn wait_for_available(&self) -> Result<(), TimelineError> {
        if self.is_available() {
            Ok(())
        } else {
            Err(TimelineError::NotAvailable)
        }
    }

    /// Initialize the timeline with the given acquire and release points
    ///
    /// This resets both the acquire and release points to the specified values.
    pub fn initialize(&mut self, acquire_point: u64, release_point: u64) {
        self.0.flags = 0;
        self.0.padding = 0;
        self.0.acquire_point = acquire_point;
        self.0.release_point = release_point;
        std::sync::atomic::fence(Ordering::Release);
    }

    /// Perform DMA-BUF synchronization using this timeline
    ///
    /// This facilitates explicit synchronization for DMA-BUF sharing between producers
    /// and consumers using the fence synchronization mechanism in the Linux DRM subsystem.

    // FIXME: This is a placeholder implementation
    #[allow(unused)]
    pub fn sync_dma_buf(&mut self, dma_buf_fd: RawFd) -> Result<(), TimelineError> {
        // In a real implementation, we would use the dmabuf_import_sync_file function
        // from the render_dmabuf.h to perform fence-based synchronization

        // Set flag to indicate synchronization
        self.set_flags(self.flags() | 1);

        // This would be the real implementation:
        // let sync_file_fd = export_sync_file(dma_buf_fd, DMA_BUF_SYNC_WRITE)?;
        // if !dmabuf_import_sync_file(log, dma_buf_fd, DMA_BUF_SYNC_WRITE, sync_file_fd) {
        //     return Err(TimelineError::SyncFailed(std::io::Error::new(
        //         std::io::ErrorKind::Other, "Failed to import sync file"
        //     )));
        // }

        Ok(())
    }

    /// Export a fence for this timeline to a sync file
    ///
    /// This creates a sync file from the timeline's acquire point that can be
    /// used to synchronize with GPU operations.
    ///
    // FIXME: This is a placeholder implementation
    #[allow(unused)]
    pub fn export_sync_file(&self, dma_buf_fd: RawFd) -> Result<RawFd, TimelineError> {
        // In a real implementation, we would use dmabuf_export_sync_file
        // to export a sync file from the DMA-BUF

        // let sync_file_fd = dmabuf_export_sync_file(log, dma_buf_fd, DMA_BUF_SYNC_READ);
        // if sync_file_fd < 0 {
        //     return Err(TimelineError::SyncFailed(std::io::Error::last_os_error()));
        // }
        // return Ok(sync_file_fd);

        // For now, return placeholder error
        Err(TimelineError::InvalidOperation)
    }

    /// Import a sync file to synchronize with this timeline
    ///
    /// This integrates external synchronization primitives with the timeline.
    #[allow(unused)]
    pub fn import_sync_file(&mut self, sync_file_fd: RawFd) -> Result<(), TimelineError> {
        // In a real implementation, we would use dmabuf_import_sync_file
        // to import the sync file into the DMA-BUF

        // Mark the buffer as synchronized by setting a flag
        self.set_flags(self.flags() | 1);

        Ok(())
    }
}

impl<'a> Debug for SyncTimelineRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncTimelineRef")
            .field("flags", &self.flags())
            .field("acquire_point", &self.acquire_point())
            .field("release_point", &self.release_point())
            .finish()
    }
}

/// A thread-safe wrapper for a spa_meta_sync_timeline
///
/// This provides thread-safe access to the timeline points without requiring
/// exclusive mutable access, which is useful for highly concurrent scenarios.
pub struct AtomicSyncTimeline {
    inner: *mut spa_sys::spa_meta_sync_timeline,
}

impl AtomicSyncTimeline {
    /// Create a new AtomicSyncTimeline from a timeline reference
    ///
    /// # Safety
    /// The provided timeline reference must outlive this AtomicSyncTimeline
    pub unsafe fn new(timeline: &mut SyncTimelineRef) -> Self {
        Self {
            inner: timeline.as_mut_ptr(),
        }
    }

    /// Create a new AtomicSyncTimeline from a raw pointer
    ///
    /// # Safety
    /// The provided pointer must be valid and point to a properly allocated
    /// timeline that outlives this AtomicSyncTimeline
    pub unsafe fn from_raw(ptr: *mut spa_sys::spa_meta_sync_timeline) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { inner: ptr })
        }
    }

    /// Set the acquire point atomically
    pub fn set_acquire_point(&self, point: u64) {
        unsafe {
            std::sync::atomic::fence(Ordering::SeqCst);
            (*self.inner).acquire_point = point;
            std::sync::atomic::fence(Ordering::SeqCst);
        }
    }

    /// Set the release point atomically
    pub fn set_release_point(&self, point: u64) {
        unsafe {
            std::sync::atomic::fence(Ordering::SeqCst);
            (*self.inner).release_point = point;
            std::sync::atomic::fence(Ordering::SeqCst);
        }
    }

    /// Get the acquire point atomically
    pub fn acquire_point(&self) -> u64 {
        unsafe {
            std::sync::atomic::fence(Ordering::SeqCst);
            let result = (*self.inner).acquire_point;
            std::sync::atomic::fence(Ordering::SeqCst);
            result
        }
    }

    /// Get the release point atomically
    pub fn release_point(&self) -> u64 {
        unsafe {
            std::sync::atomic::fence(Ordering::SeqCst);
            let result = (*self.inner).release_point;
            std::sync::atomic::fence(Ordering::SeqCst);
            result
        }
    }
}

// These impls are safe because we're only accessing the timeline
// through atomic operations with memory barriers
unsafe impl Send for AtomicSyncTimeline {}
unsafe impl Sync for AtomicSyncTimeline {}
