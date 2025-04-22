use super::stream::StreamRef;

use spa::buffer::Data;
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
}

impl<'s> Drop for Buffer<'s> {
    fn drop(&mut self) {
        unsafe {
            self.stream.queue_raw_buffer(self.buf.as_ptr());
        }
    }
}
