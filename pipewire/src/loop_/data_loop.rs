// Copyright The pipewire-rs Contributors.
// SPDX-License-Identifier: MIT

//! PipeWire DataLoop implementation

use std::ptr::{self, NonNull};
use spa::utils::dict::DictRef;
use crate::Error;
use crate::properties::Properties;

/// A DataLoop for PipeWire processing
#[derive(Debug)]
pub struct DataLoop {
    ptr: NonNull<pw_sys::pw_data_loop>,
    owns_ptr: bool,
}
#[allow(dead_code)]
impl DataLoop {
    /// Create a new DataLoop with the specified properties
    pub fn new(properties: Option<&Properties>) -> Result<Self, Error> {
        let props_ptr = match properties {
            Some(props) => {
                // Assuming `Properties` derefs or AsRef's to `DictRef`
                // Get the DictRef first
                let dict_ref: &DictRef = props.as_ref(); // Or props.deref() if it uses Deref
                // Then get the raw pointer from DictRef
                dict_ref.as_raw()
            }
            None => ptr::null(),
        };

        let ptr = unsafe { pw_sys::pw_data_loop_new(props_ptr) };
        if ptr.is_null() {
            return Err(Error::CreationFailed);
        }

        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            owns_ptr: true,
        })
    }
    /// Get the underlying loop
    pub fn get_loop(&self) -> &super::LoopRef {
        unsafe {
            let loop_ptr = pw_sys::pw_data_loop_get_loop(self.ptr.as_ptr());
            &*(loop_ptr as *const super::LoopRef)
        }
    }

    /// Start the data loop thread
    pub fn start(&self) -> Result<(), Error> {
        let res = unsafe { pw_sys::pw_data_loop_start(self.ptr.as_ptr()) };
        if res < 0 {
            Err(Error::from(res))
        } else {
            Ok(())
        }
    }

    /// Stop the data loop thread
    pub fn stop(&self) -> Result<(), Error> {
        let res = unsafe { pw_sys::pw_data_loop_stop(self.ptr.as_ptr()) };
        if res < 0 {
            Err(Error::from(res))
        } else {
            Ok(())
        }
    }

    /// Check if we're in the data loop thread
    pub fn in_thread(&self) -> bool {
        unsafe {
            // Convert the result to bool explicitly
            pw_sys::pw_data_loop_in_thread(self.ptr.as_ptr()) != false
        }
    }

    /// Get the name of the data loop
    ///
    /// Note: This accesses the underlying loop's name
    pub fn name(&self) -> Option<&str> {
        self.get_loop().name()
    }

    /// Signal the data loop to exit
    pub fn exit(&self) {
        unsafe {
            pw_sys::pw_data_loop_exit(self.ptr.as_ptr());
        }
    }
}

impl Drop for DataLoop {
    fn drop(&mut self) {
        if self.owns_ptr {
            unsafe {
                pw_sys::pw_data_loop_destroy(self.ptr.as_ptr());
            }
        }
    }
}
