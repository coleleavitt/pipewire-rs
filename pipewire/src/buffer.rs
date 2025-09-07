use super::stream::StreamRef;

use spa::buffer::{Data, DataType, SyncTimelineRef};
use std::convert::TryFrom;
use std::ptr::NonNull;

/// A buffer for a stream.
///
/// A buffer contains data that was dequeued from a stream. Buffers are used
/// for transferring media between the client and server, and can be either
/// input or output buffers.
///
/// When a Buffer is dropped, it is automatically queued back to the stream.
pub struct Buffer<'s> {
    buf: NonNull<pw_sys::pw_buffer>,

    /// In Pipewire, buffers are owned by the stream that generated them.
    /// This reference ensures that this rule is respected.
    stream: &'s StreamRef,
}

impl<'s> Buffer<'s> {
    /// Creates a Buffer from a raw pw_buffer pointer and associated stream
    ///
    /// # Safety
    /// The buffer pointer must be a valid pw_buffer obtained from the specified stream
    pub(crate) unsafe fn from_raw(
        buf: *mut pw_sys::pw_buffer,
        stream: &'s StreamRef,
    ) -> Option<Buffer<'s>> {
        NonNull::new(buf).map(|buf| Buffer { buf, stream })
    }

    /// Provides mutable access to the buffer data
    ///
    /// Returns a mutable slice of the buffer's data elements. Each data element
    /// corresponds to a plane of data in the buffer.
    pub fn datas_mut(&mut self) -> &mut [Data] {
        let buffer: *mut spa_sys::spa_buffer = unsafe { self.buf.as_ref().buffer };

        let slice_of_data = if !buffer.is_null()
            && unsafe { (*buffer).n_datas > 0 && !(*buffer).datas.is_null() }
        {
            unsafe {
                let datas = (*buffer).datas as *mut Data;
                std::slice::from_raw_parts_mut(datas, usize::try_from((*buffer).n_datas).unwrap())
            }
        } else {
            &mut []
        };

        slice_of_data
    }
    #[cfg(feature = "v0_3_49")]
    pub fn requested(&self) -> u64 {
        unsafe { self.buf.as_ref().requested }
    }

    /// Gets sync timeline metadata from the buffer if present
    pub fn get_sync_timeline_metadata(&self) -> Option<SyncTimelineRef> {
        let buffer: *mut spa_sys::spa_buffer = unsafe { self.buf.as_ref().buffer };
        
        if buffer.is_null() {
            return None;
        }

        let spa_buffer = unsafe { &*buffer };
        if spa_buffer.n_metas == 0 || spa_buffer.metas.is_null() {
            return None;
        }

        // Search through metadata for sync timeline
        for i in 0..spa_buffer.n_metas {
            let meta = unsafe { &*spa_buffer.metas.add(i as usize) };
            
            // Check if this meta is a sync timeline (spa_meta_type_SPA_META_SyncTimeline = 9)
            if meta.type_ == 9 && !meta.data.is_null() { // spa_meta_type_SPA_META_SyncTimeline
                unsafe {
                    let sync_timeline_ptr = meta.data as *mut spa_sys::spa_meta_sync_timeline;
                    return SyncTimelineRef::from_raw(sync_timeline_ptr);
                }
            }
        }
        
        None
    }

    /// Gets all DMA-BUF data elements from this buffer
    pub fn datas_with_type(&self, data_type: DataType) -> Option<Vec<&Data>> {
        let mut matching_datas = Vec::new();
        
        let buffer: *mut spa_sys::spa_buffer = unsafe { self.buf.as_ref().buffer };
        if buffer.is_null() {
            return None;
        }

        let spa_buffer = unsafe { &*buffer };
        if spa_buffer.n_datas == 0 || spa_buffer.datas.is_null() {
            return None;
        }

        for i in 0..spa_buffer.n_datas {
            let data = unsafe { &*(spa_buffer.datas.add(i as usize) as *const Data) };
            if data.type_() == data_type {
                matching_datas.push(data);
            }
        }
        
        if matching_datas.is_empty() {
            None
        } else {
            Some(matching_datas)
        }
    }

    /// Helper method to get sync object file descriptors from sync data
    pub fn get_sync_fds(&self) -> Option<(std::os::unix::io::RawFd, std::os::unix::io::RawFd)> {
        if let Some(sync_datas) = self.datas_with_type(DataType::SyncObj) {
            if sync_datas.len() >= 2 {
                let acquire_fd = sync_datas[0].fd()?;  // First syncobj is acquire timeline
                let release_fd = sync_datas[1].fd()?;  // Second syncobj is release timeline
                return Some((acquire_fd, release_fd));
            }
        }
        None
    }

    /// Convert buffer back to raw pointer for queuing, consuming the Buffer
    pub(crate) fn into_raw(self) -> *mut pw_sys::pw_buffer {
        let buf_ptr = self.buf.as_ptr();
        std::mem::forget(self); // Don't drop, we're transferring ownership
        buf_ptr
    }
}

impl<'s> Drop for Buffer<'s> {
    fn drop(&mut self) {
        unsafe {
            self.stream.queue_raw_buffer(self.buf.as_ptr());
        }
    }
}
