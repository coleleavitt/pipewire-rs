//! # DRM Syncobj Timeline Integration for linux-drm-syncobj-v1
//!
//! This module provides proper Linux DRM syncobj timeline integration for PipeWire's
//! explicit synchronization support. It implements the kernel interfaces needed for
//! the linux-drm-syncobj-v1 Wayland protocol.
//!
//! ## Protocol Overview
//! 
//! The linux-drm-syncobj-v1 protocol enables explicit synchronization using DRM
//! synchronization objects with timeline semantics. Unlike binary fences, timeline
//! syncobjs use incrementing point values that allow complex dependency tracking.
//!
//! ## File Descriptor Flow
//!
//! 1. **Wayland Protocol**: Client imports DRM syncobj timeline FDs via 
//!    `wp_linux_drm_syncobj_manager_v1.import_timeline`
//! 2. **PipeWire Integration**: These timeline FDs are passed through PipeWire's
//!    `spa_meta_sync_timeline` metadata with acquire/release points
//! 3. **DRM Kernel Interface**: Our implementation converts timeline FDs to DRM
//!    handles and performs actual syncobj timeline operations via ioctls
//!
//! ## Key Functions
//!
//! - `fd_to_drm_handle()`: Converts syncobj fd to DRM handle using `DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE`
//! - `drm_syncobj_timeline_wait()`: Waits for timeline point via `DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT`  
//! - `drm_syncobj_timeline_signal()`: Signals timeline point via `DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL`

use libc::{c_int, c_ulong, ioctl};
use std::os::unix::io::RawFd;
use std::ptr;

// DRM syncobj query flags
const DRM_SYNCOBJ_QUERY_FLAGS_LAST_SUBMITTED: u32 = 1 << 0;

// DRM syncobj creation flags
const DRM_SYNCOBJ_CREATE_SIGNALED: u32 = 1 << 0;

// DRM ioctl calculation macros matching kernel headers
const DRM_IOC_NONE: c_ulong = 0;
const DRM_IOC_READ: c_ulong = 2;
const DRM_IOC_WRITE: c_ulong = 1;
const DRM_IOC_READWRITE: c_ulong = DRM_IOC_READ | DRM_IOC_WRITE;

const fn drm_iowr(nr: c_ulong, size: usize) -> c_ulong {
    (DRM_IOC_READWRITE << 30) | ((size as c_ulong) << 16) | (0x64 << 8) | nr
}

// DRM ioctl constants calculated properly from kernel headers
const DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT: c_ulong = drm_iowr(0xCA, std::mem::size_of::<DrmSyncobjTimelineWait>());
const DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL: c_ulong = drm_iowr(0xCD, std::mem::size_of::<DrmSyncobjTimelineArray>());
const DRM_IOCTL_SYNCOBJ_QUERY: c_ulong = drm_iowr(0xCB, std::mem::size_of::<DrmSyncobjTimelineArray>());
const DRM_IOCTL_SYNCOBJ_EVENTFD: c_ulong = drm_iowr(0xCF, std::mem::size_of::<DrmSyncobjEventfd>());
const DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE: c_ulong = drm_iowr(0xC2, std::mem::size_of::<DrmSyncobjHandle>());
const DRM_IOCTL_SYNCOBJ_CREATE: c_ulong = drm_iowr(0xBF, std::mem::size_of::<DrmSyncobjCreate>());
const DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD: c_ulong = drm_iowr(0xC1, std::mem::size_of::<DrmSyncobjHandle>());
const DRM_IOCTL_VERSION: c_ulong = drm_iowr(0x00, std::mem::size_of::<DrmVersion>());

// DRM syncobj structures matching the kernel headers exactly
#[repr(C)]
struct DrmSyncobjTimelineWait {
    handles: u64,       // pointer to array of handles
    points: u64,        // pointer to array of timeline points  
    timeout_nsec: i64,  // absolute timeout in nanoseconds
    count_handles: u32, // number of handles
    flags: u32,         // wait flags
    first_signaled: u32,// index of first signaled (output)
    pad: u32,           // padding
    deadline_nsec: u64, // fence deadline hint (added in newer kernels)
}

#[repr(C)]
struct DrmSyncobjTimelineArray {
    handles: u64,       // pointer to array of handles
    points: u64,        // pointer to array of timeline points
    count_handles: u32, // number of handles
    flags: u32,         // flags
}

/// Wait for DRM syncobj timeline points to be signaled
/// 
/// This implements the actual kernel interface for waiting on DRM syncobj
/// timeline points as used by the linux-drm-syncobj-v1 protocol.
pub fn drm_syncobj_timeline_wait(
    drm_fd: RawFd,
    handle: u32,
    point: u64,
    timeout_nsec: i64,
) -> Result<(), std::io::Error> {
    let handle_ptr = &handle as *const u32 as u64;
    let point_ptr = &point as *const u64 as u64;
    
    let wait_args = DrmSyncobjTimelineWait {
        handles: handle_ptr,
        points: point_ptr,
        timeout_nsec,
        count_handles: 1,
        flags: 0,
        first_signaled: 0,
        pad: 0,
        deadline_nsec: 0, // No deadline hint for now
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT,
            &wait_args as *const _ as *const libc::c_void,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Signal DRM syncobj timeline points
///
/// This implements the actual kernel interface for signaling DRM syncobj
/// timeline points as used by the linux-drm-syncobj-v1 protocol.
pub fn drm_syncobj_timeline_signal(
    drm_fd: RawFd,
    handle: u32,
    point: u64,
) -> Result<(), std::io::Error> {
    let handle_ptr = &handle as *const u32 as u64;
    let point_ptr = &point as *const u64 as u64;
    
    let signal_args = DrmSyncobjTimelineArray {
        handles: handle_ptr,
        points: point_ptr,
        count_handles: 1,
        flags: 0,
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL,
            &signal_args as *const _ as *const libc::c_void,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[repr(C)]
struct DrmSyncobjEventfd {
    handle: u32,
    flags: u32,
    point: u64,
    fd: i32,
    pad: u32,
}

#[repr(C)]
struct DrmVersion {
    version_major: c_int,
    version_minor: c_int,
    version_patchlevel: c_int,
    name_len: libc::size_t,
    name: *mut libc::c_char,
    date_len: libc::size_t,
    date: *mut libc::c_char,
    desc_len: libc::size_t,
    desc: *mut libc::c_char,
}

/// Check if a file descriptor is a valid DRM device
pub fn is_drm_fd(fd: RawFd) -> bool {
    // Try to get DRM version info to verify it's a DRM device
    
    let mut version = DrmVersion {
        version_major: 0,
        version_minor: 0,
        version_patchlevel: 0,
        name_len: 0,
        name: ptr::null_mut(),
        date_len: 0,
        date: ptr::null_mut(),
        desc_len: 0,
        desc: ptr::null_mut(),
    };
    
    let ret = unsafe {
        ioctl(fd, DRM_IOCTL_VERSION, &mut version as *mut _ as *mut libc::c_void)
    };
    
    ret == 0
}

#[repr(C)]
struct DrmSyncobjHandle {
    handle: u32,
    flags: u32,
    fd: i32,
    pad: u32,
}

#[repr(C)]
struct DrmSyncobjCreate {
    handle: u32,
    flags: u32,
}

/// Extract DRM handle from syncobj file descriptor
///
/// Converts a syncobj file descriptor to a DRM handle using the proper DRM ioctl.
/// The file descriptor should be a DRM syncobj fd that was exported from another
/// process or imported via DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD.
pub fn fd_to_drm_handle(drm_device_fd: RawFd, syncobj_fd: RawFd) -> Result<u32, std::io::Error> {
    
    if !is_drm_fd(drm_device_fd) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Not a valid DRM device file descriptor"
        ));
    }
    
    let mut handle_args = DrmSyncobjHandle {
        handle: 0,  // Output - will be filled by kernel
        flags: 0,   // No special flags needed
        fd: syncobj_fd,
        pad: 0,
    };
    
    let ret = unsafe {
        ioctl(
            drm_device_fd,
            DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE,
            &mut handle_args as *mut _ as *mut libc::c_void,
        )
    };
    
    if ret == 0 {
        Ok(handle_args.handle)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Managed DRM device file descriptor
/// 
/// This struct properly manages the lifetime of a DRM device file descriptor
/// to prevent premature closing and resource leaks.
pub struct DrmDeviceFd {
    fd: RawFd,
}

impl DrmDeviceFd {
    pub fn new(fd: RawFd) -> Self {
        Self { fd }
    }
    
    pub fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for DrmDeviceFd {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

/// Find the DRM device file descriptor associated with a syncobj fd
/// 
/// In practice, PipeWire should provide both the DRM device fd and the syncobj fds
/// together as part of the buffer negotiation. This is a fallback that attempts
/// to find a suitable DRM device and returns a properly managed DrmDeviceFd.
pub fn find_drm_device_fd() -> Result<DrmDeviceFd, std::io::Error> {
    // Try common DRM device nodes
    for i in 0..16 {
        let path = format!("/dev/dri/card{}", i);
        if let Ok(file) = std::fs::File::open(&path) {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            if is_drm_fd(fd) {
                // Duplicate the file descriptor to manage its lifetime properly
                let fd_copy = unsafe { libc::dup(fd) };
                return if fd_copy >= 0 {
                    Ok(DrmDeviceFd::new(fd_copy))
                } else {
                    Err(std::io::Error::last_os_error())
                };
            }
        }
    }
    
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No DRM device found"
    ))
}

/// Query DRM syncobj timeline points
///
/// This queries the current signaled timeline point for a DRM syncobj timeline.
/// Used to check if a timeline point has been signaled without blocking.
pub fn drm_syncobj_timeline_query(
    drm_fd: RawFd,
    handle: u32,
) -> Result<u64, std::io::Error> {
    let handle_ptr = &handle as *const u32 as u64;
    let mut point: u64 = 0;
    let point_ptr = &mut point as *mut u64 as u64;
    
    let query_args = DrmSyncobjTimelineArray {
        handles: handle_ptr,
        points: point_ptr,
        count_handles: 1,
        flags: DRM_SYNCOBJ_QUERY_FLAGS_LAST_SUBMITTED,
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_QUERY,
            &query_args as *const _ as *const libc::c_void,
        )
    };

    if ret == 0 {
        Ok(point)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Register an eventfd for syncobj timeline notification
///
/// This registers an eventfd to be signaled when a specific timeline point
/// on a syncobj timeline is reached. This enables proper async notification
/// instead of polling.
pub fn drm_syncobj_eventfd_register(
    drm_fd: RawFd,
    handle: u32,
    timeline_point: u64,
    event_fd: RawFd,
) -> Result<(), std::io::Error> {
    let eventfd_args = DrmSyncobjEventfd {
        handle,
        flags: 0, // Wait for point to be signaled
        point: timeline_point,
        fd: event_fd,
        pad: 0,
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_EVENTFD,
            &eventfd_args as *const _ as *const libc::c_void,
        )
    };

    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Create a new DRM syncobj timeline object
///
/// Creates a new syncobj timeline that can be used for explicit synchronization.
/// Timeline syncobjs use incrementing point values rather than binary states.
pub fn create_drm_syncobj_timeline(drm_fd: RawFd) -> Result<u32, std::io::Error> {
    let mut create_args = DrmSyncobjCreate {
        handle: 0, // Output - will be filled by kernel
        flags: 0,  // Create unsignaled timeline
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_CREATE,
            &mut create_args as *mut _ as *mut libc::c_void,
        )
    };

    if ret == 0 {
        Ok(create_args.handle)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Export DRM syncobj handle to file descriptor
///
/// Converts a DRM syncobj handle to a file descriptor that can be passed
/// to other processes or used in PipeWire timeline metadata.
pub fn drm_syncobj_handle_to_fd(drm_fd: RawFd, handle: u32) -> Result<RawFd, std::io::Error> {
    let mut handle_args = DrmSyncobjHandle {
        handle,
        flags: 0,
        fd: -1, // Output - will be filled by kernel
        pad: 0,
    };

    let ret = unsafe {
        ioctl(
            drm_fd,
            DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD,
            &mut handle_args as *mut _ as *mut libc::c_void,
        )
    };

    if ret == 0 {
        Ok(handle_args.fd)
    } else {
        Err(std::io::Error::last_os_error())
    }
}