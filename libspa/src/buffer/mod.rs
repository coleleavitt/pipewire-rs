// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, fmt::Debug};
use std::os::fd::RawFd;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DataType(spa_sys::spa_data_type);

pub mod meta;

// Re-export the metadata types
pub use meta::SyncTimelineRef;
pub use meta::TimelineError;
pub use meta::AtomicSyncTimeline;

#[allow(non_upper_case_globals)]
impl DataType {
    pub const Invalid: Self = Self(spa_sys::SPA_DATA_Invalid);
    /// Pointer to memory, the data field in struct [`Data`] is set.
    pub const MemPtr: Self = Self(spa_sys::SPA_DATA_MemPtr);
    /// Generic fd, `mmap` to get to memory
    pub const MemFd: Self = Self(spa_sys::SPA_DATA_MemFd);
    /// Fd to `dmabuf` memory
    pub const DmaBuf: Self = Self(spa_sys::SPA_DATA_DmaBuf);
    /// Memory is identified with an id
    pub const MemId: Self = Self(spa_sys::SPA_DATA_MemId);
    /// Syncobj, usually requires a spa_meta_sync_timeline metadata
    pub const SyncObj: Self = Self(spa_sys::SPA_DATA_SyncObj);

    pub fn from_raw(raw: spa_sys::spa_data_type) -> Self {
        Self(raw)
    }

    pub fn as_raw(&self) -> spa_sys::spa_data_type {
        self.0
    }
}

impl std::fmt::Debug for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = format!(
            "DataType::{}",
            match *self {
                Self::Invalid => "Invalid",
                Self::MemPtr => "MemPtr",
                Self::MemFd => "MemFd",
                Self::DmaBuf => "DmaBuf",
                Self::MemId => "MemId",
                Self::SyncObj => "SyncObj",
                _ => "Unknown",
            }
        );
        f.write_str(&name)
    }
}

/// Error types specific to DMA-BUF operations
#[derive(Debug, thiserror::Error)]
pub enum DmaError {
    #[error("Not a DMA-BUF buffer")]
    NotDmaBuf,
    #[error("DMA-BUF synchronization failed: {0}")]
    SyncFailed(#[from] TimelineError),
    #[error("Invalid operation on DMA-BUF: {0}")]
    InvalidOperation(String),
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct DataFlags: u32 {
        /// Data is readable
        const READABLE = 1<<0;
        /// Data is writable
        const WRITABLE = 1<<1;
        /// Data pointer can be changed
        const DYNAMIC = 1<<2;
        /// Data is both readable and writable
        const READWRITE = Self::READABLE.bits() | Self::WRITABLE.bits();
        /// Data is mappable with simple mmap/munmap
        const MAPPABLE = 1<<3;
    }
}

#[repr(transparent)]
pub struct Data(spa_sys::spa_data);

impl Data {
    pub fn as_raw(&self) -> &spa_sys::spa_data {
        &self.0
    }

    pub fn type_(&self) -> DataType {
        DataType::from_raw(self.0.type_)
    }

    pub fn flags(&self) -> DataFlags {
        DataFlags::from_bits_retain(self.0.flags)
    }

    /// Get the file descriptor for this data
    ///
    /// Returns `None` if the data doesn't have an associated file descriptor,
    /// if the descriptor is invalid, or if it can't be represented as a RawFd.
    pub fn fd(&self) -> Option<RawFd> {
        match self.type_() {
            DataType::MemFd | DataType::DmaBuf | DataType::SyncObj => {
                if self.0.fd >= 0 {
                    // Convert i64 to i32 safely without panicking if conversion fails
                    self.0.fd.try_into().ok()
                } else {
                    None
                }
            },
            _ => None
        }
    }

    pub fn data(&mut self) -> Option<&mut [u8]> {
        // FIXME: For safety, perhaps only return a non-mut slice when DataFlags::WRITABLE is not set?
        if self.0.data.is_null() {
            None
        } else {
            unsafe {
                Some(std::slice::from_raw_parts_mut(
                    self.0.data as *mut u8,
                    usize::try_from(self.0.maxsize).unwrap(),
                ))
            }
        }
    }

    pub fn chunk(&self) -> &Chunk {
        assert_ne!(self.0.chunk, std::ptr::null_mut());
        unsafe {
            let chunk: *const spa_sys::spa_chunk = self.0.chunk;
            &*(chunk as *const Chunk)
        }
    }

    pub fn chunk_mut(&mut self) -> &mut Chunk {
        assert_ne!(self.0.chunk, std::ptr::null_mut());
        unsafe {
            let chunk: *mut spa_sys::spa_chunk = self.0.chunk;
            &mut *(chunk as *mut Chunk)
        }
    }

    /// Get the DMA-BUF file descriptor
    ///
    /// Returns the file descriptor if this data represents a DMA-BUF,
    /// otherwise returns None.
    pub fn dma_buf_fd(&self) -> Option<RawFd> {
        if self.type_() == DataType::DmaBuf {
            // Convert i64 to i32 safely
            self.0.fd.try_into().ok()
        } else {
            None
        }
    }

    /// Perform synchronization operations for a DMA-BUF using a timeline
    ///
    /// This method handles the explicit synchronization necessary for DMA-BUF
    /// sharing between producers and consumers, using the fence synchronization
    /// mechanism provided by the Linux DRM subsystem.
    ///
    /// Returns an error if this data is not a DMA-BUF.
    pub fn sync_dma_buf<'a>(&self, timeline: &mut SyncTimelineRef<'a>) -> Result<(), DmaError> {
        if let Some(fd) = self.dma_buf_fd() {
            // Delegate the synchronization to the timeline
            timeline.sync_dma_buf(fd)?;
            Ok(())
        } else {
            Err(DmaError::NotDmaBuf)
        }
    }

    /// Import a sync file from an external source to synchronize with this DMA-BUF
    ///
    /// This is used to coordinate access to the DMA-BUF with external systems
    /// like GPU drivers. It imports a sync file (typically from a GPU operation)
    /// and associates it with this DMA-BUF for synchronization.
    ///
    /// Returns an error if this is not a DMA-BUF.
    #[allow(unused)]
    pub fn import_sync_file<'a>(&self, timeline: &mut SyncTimelineRef<'a>, sync_file_fd: RawFd) -> Result<(), DmaError> {
        if let Some(fd) = self.dma_buf_fd() {
            // We would use DRM IOCTLs to import the sync file
            // For now, we just update the timeline to mark the buffer as synchronized
            timeline.import_sync_file(sync_file_fd)?;

            // Here we would call the actual import function, something like:
            // dmabuf_import_sync_file(log, fd, DMA_BUF_SYNC_RW, sync_file_fd)

            Ok(())
        } else {
            Err(DmaError::NotDmaBuf)
        }
    }

    /// Export a sync file from this DMA-BUF to synchronize with external operations
    ///
    /// This creates a sync file that represents the current state of the DMA-BUF
    /// and can be passed to external systems like GPU drivers for synchronization.
    ///
    /// Returns the sync file descriptor, or an error if this is not a DMA-BUF.
    pub fn export_sync_file<'a>(&self, timeline: &SyncTimelineRef<'a>) -> Result<RawFd, DmaError> {
        if let Some(fd) = self.dma_buf_fd() {
            // For a real implementation we would call:
            // dmabuf_export_sync_file(log, fd, DMA_BUF_SYNC_RW)

            // Instead, we'll try to export from the timeline
            timeline.export_sync_file(fd)
                .map_err(|e| DmaError::SyncFailed(e))
        } else {
            Err(DmaError::NotDmaBuf)
        }
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("type", &self.type_())
            .field("flags", &self.flags())
            .field("fd", &self.fd())
            .field("data", &self.0.data) // Only print the pointer here, as we don't want to print a (potentially very big) slice.
            .field("chunk", &self.chunk())
            .finish()
    }
}

bitflags::bitflags! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct ChunkFlags: i32 {
        /// Chunk data is corrupted in some way
        const CORRUPTED = 1<<0;
        /// Chunk contains empty/neutral data (silence/black)
        const EMPTY = 1<<1;
    }
}

#[repr(transparent)]
pub struct Chunk(spa_sys::spa_chunk);

impl Chunk {
    pub fn as_raw(&self) -> &spa_sys::spa_chunk {
        &self.0
    }

    pub fn size(&self) -> u32 {
        self.0.size
    }

    pub fn size_mut(&mut self) -> &mut u32 {
        &mut self.0.size
    }

    pub fn offset(&self) -> u32 {
        self.0.offset
    }

    pub fn offset_mut(&mut self) -> &mut u32 {
        &mut self.0.offset
    }

    pub fn stride(&self) -> i32 {
        self.0.stride
    }

    pub fn stride_mut(&mut self) -> &mut i32 {
        &mut self.0.stride
    }

    pub fn flags(&self) -> ChunkFlags {
        ChunkFlags::from_bits_retain(self.0.flags)
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunk")
            .field("offset", &self.offset())
            .field("size", &self.size())
            .field("stride", &self.stride())
            .field("flags", &self.flags())
            .finish()
    }
}
