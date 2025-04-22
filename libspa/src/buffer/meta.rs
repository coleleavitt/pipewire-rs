use spa_sys::spa_meta_sync_timeline;
use std::fmt::Debug;
use std::sync::Arc;

/// A transparent wrapper around a spa_meta_sync_timeline for explicit synchronization.
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
        if self.acquire_point() >= self.release_point() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("The buffer is not available yet."))
        }
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
