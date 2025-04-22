// Copyright The pipewire-rs Contributors.

// SPDX-License-Identifier: MIT

//! # Rust bindings for pipewire
//! `pipewire` is a crate offering a rustic bindings for `libpipewire`, the library for interacting
//! with the pipewire server.
//!
//! Programs that interact with pipewire usually react to events from the server by registering callbacks
//! and invoke methods on objects on the server by calling methods on local proxy objects.
//!
//! ## Getting started
//! Most programs that interact with pipewire will need the same few basic objects:
//! - A [`MainLoop`](`main_loop::MainLoop`) that drives the program, reacting to any incoming events and dispatching method calls.
//!   Most of a time, the program/thread will sit idle in this loop, waiting on events to occur.
//! - A [`Context`](`context::Context`) that keeps track of any pipewire resources.
//! - A [`Core`](`core::Core`) that is a proxy for the remote pipewire instance, used to send messages to and receive events from the
//!   remote server.
//! - Optionally, a [`Registry`](`registry::Registry`) that can be used to manage and track available objects on the server.
//!
//! This is how they can be created:
// ignored because https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/issues/19
//! ```
//! use pipewire::{main_loop::MainLoop, context::Context};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mainloop = MainLoop::new(None)?;
//!     let context = Context::new(&mainloop)?;
//!     let core = context.connect(None)?;
//!     let registry = core.get_registry()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! Now you can start hooking up different kinds of callbacks to the objects to react to events, and call methods
//! on objects to change the state of the remote.
//! ```
//! use pipewire::{main_loop::MainLoop, context::Context};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mainloop = MainLoop::new(None)?;
//!     let context = Context::new(&mainloop)?;
//!     let core = context.connect(None)?;
//!     let registry = core.get_registry()?;
//!
//!     // Register a callback to the `global` event on the registry, which notifies of any new global objects
//!     // appearing on the remote.
//!     // The callback will only get called as long as we keep the returned listener alive.
//!     let _listener = registry
//!         .add_listener_local()
//!         .global(|global| println!("New global: {:?}", global))
//!         .register();
//!
//!     // Calling the `destroy_global` method on the registry will destroy the object with the specified id on the remote.
//!     // We don't have a specific object to destroy now, so this is commented out.
//!     # // FIXME: Find a better method for this example we can actually call.
//!     // registry.destroy_global(313).into_result()?;
//!
//!     mainloop.run();
//!
//!     Ok(())
//! }
//! ```
//! Note that registering any callback requires the closure to have the `'static` lifetime, so if you need to capture
//! any variables, use `move ||` closures, and use `std::rc::Rc`s to access shared variables
//! and some `std::cell` variant if you need to mutate them.
//!
//! Also note that we called `mainloop.run()` at the end.
//! This will enter the loop, and won't return until we call `mainloop.quit()` from some event.
//! If we didn't run the loop, events and method invocations would not be processed, so the program would terminate
//! without doing much.
//!
//! ## The main loop
//! Sometimes, other stuff needs to be done even though we are waiting inside the main loop. \
//! This can be done by adding sources to the loop.
//!
//! For example, we can call a function on an interval:
//!
//! ```
//! use pipewire::main_loop::MainLoop;
//! use std::time::Duration;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mainloop = MainLoop::new(None)?;
//!
//!     let timer = mainloop.loop_().add_timer(|_| println!("Hello"));
//!     // Call the first time in half a second, and then in a one second interval.
//!     timer.update_timer(Some(Duration::from_millis(500)), Some(Duration::from_secs(1))).into_result()?;
//!
//!     mainloop.run();
//!
//!     Ok(())
//! }
//! ```
//! This program will print out "Hello" every second forever.
//!
//! Using similar methods, you can also react to IO or Signals, or call a callback whenever the loop is idle.
//!
//! ## Multithreading
//! The pipewire library is not really thread-safe, so pipewire objects do not implement [`Send`](`std::marker::Send`)
//! or [`Sync`](`std::marker::Sync`).
//!
//! However, you can spawn a [`MainLoop`](`main_loop::MainLoop`) in another thread and do bidirectional communication using two channels.
//!
//! To send messages to the main thread, we can easily use a [`std::sync::mpsc`].
//! Because we are stuck in the main loop in the pipewire thread and can't just block on receiving a message,
//! we use a [`pipewire::channel`](`crate::channel`) instead.
//!
//! See the [`pipewire::channel`](`crate::channel`) module for details.
//!
//! ## Asynchronous API (requires `async` feature)
//!
//! PipeWire 1.2 introduces native asynchronous processing with explicit sync support.
//! When the `async` feature is enabled, this crate provides a Rust async/await compatible API that
//! directly maps to PipeWire's async model.
//!
//! The async API maintains compatibility with safety-critical requirements:
//! - Bounded execution guarantees through explicit timeouts
//! - Predictable memory usage with fixed-size allocations
//! - Optional radiation hardening patterns for mission-critical applications
//!
//! ### Getting Started with Async API
//!
//! The async API provides equivalents to the core PipeWire objects:
//!
//! ```
//! # #[cfg(feature = "async")]
//! use futures::executor::block_on;
//! # #[cfg(feature = "async")]
//! use pipewire::async::{AsyncContext, AsyncCore};
//!
//! # #[cfg(feature = "async")]
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize PipeWire
//!     pipewire::init();
//!
//!     // Set up async context
//!     block_on(async {
//!         // Create async context
//!         let context = AsyncContext::new()?;
//!
//!         // Connect to PipeWire
//!         let core = context.connect().await?;
//!
//!         // Get registry and list objects
//!         let registry = core.get_registry().await?;
//!         let nodes = registry.list_objects::<pipewire::node::Node>().await?;
//!
//!         for node in nodes {
//!             println!("Found node: {} (id: {})", node.name(), node.id());
//!         }
//!
//!         Ok(())
//!     })
//! }
//! # #[cfg(not(feature = "async"))]
//! # fn main() { }
//! ```
//!
//! ### Working with Async Streams
//!
//! PipeWire 1.2's explicit sync metadata is exposed through the async API:
//!
//! ```
//! # #[cfg(feature = "async")]
//! use futures::executor::block_on;
//! # #[cfg(feature = "async")]
//! use pipewire::{
//!     async::{AsyncContext, AsyncStream},
//!     properties::properties,
//!     spa
//! };
//!
//! # #[cfg(feature = "async")]
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pipewire::init();
//!
//!     block_on(async {
//!         let context = AsyncContext::new()?;
//!         context.start()?;
//!
//!         // Create a stream
//!         let props = properties! {
//!             *pipewire::keys::MEDIA_TYPE => "Audio",
//!             *pipewire::keys::MEDIA_CATEGORY => "Playback",
//!         };
//!
//!         let stream = AsyncStream::new(&mut context, Some("test-stream"), props)?;
//!
//!         // Connect with format parameters
//!         let params = create_audio_format_params(44100, 2)?;
//!         stream.connect(
//!             spa::utils::Direction::Output,
//!             None,
//!             pipewire::stream::StreamFlags::AUTOCONNECT | pipewire::stream::StreamFlags::MAP_BUFFERS,
//!             &[params],
//!         ).await?;
//!
//!         // Process buffers asynchronously
//!         let buffers = stream.process().await?;
//!         for mut buffer in buffers {
//!             // Asynchronously acquire buffer with explicit sync
//!             let data = buffer.acquire().await?;
//!             // Process data...
//!             // Release buffer when done
//!             buffer.release().await?;
//!         }
//!
//!         Ok(())
//!     })
//! }
//!
//! # #[cfg(feature = "async")]
//! fn create_audio_format_params(rate: i32, channels: i32) -> Result<*mut spa_sys::spa_pod, Box<dyn std::error::Error>> {
//!     // Create audio format parameters (implementation omitted)
//!     # Ok(std::ptr::null_mut())
//! }
//! # #[cfg(not(feature = "async"))]
//! # fn main() { }
//! ```
//!
//! The async API takes full advantage of PipeWire 1.2 features like multiple data-loops,
//! CPU affinity controls, and explicit sync metadata.

pub mod buffer;
pub mod channel;
pub mod client;
pub mod constants;
pub mod context;
pub mod core;
pub mod device;
pub mod factory;
pub mod keys;
pub mod link;
pub mod loop_;
pub mod main_loop;
pub mod metadata;
pub mod module;
pub mod node;
pub mod permissions;
pub mod port;
pub mod properties;
pub mod proxy;
pub mod registry;
pub mod stream;
pub mod thread_loop;
pub mod types;

mod error;
pub use error::*;

mod utils;

pub use pw_sys as sys;
pub use spa;

// Conditionally include the async module when the async feature is enabled
#[cfg(feature = "async")]
pub mod async_v;

use std::ptr;

/// Initialize PipeWire
///
/// Initialize the PipeWire system and set up debugging
/// through the environment variable `PIPEWIRE_DEBUG`.
pub fn init() {
    use once_cell::sync::OnceCell;
    static INITIALIZED: OnceCell<()> = OnceCell::new();
    INITIALIZED.get_or_init(|| unsafe { pw_sys::pw_init(ptr::null_mut(), ptr::null_mut()) });
}

/// Deinitialize PipeWire
///
/// # Safety
/// This must only be called once during the lifetime of the process, once no PipeWire threads
/// are running anymore and all PipeWire resources are released.
pub unsafe fn deinit() {
    pw_sys::pw_deinit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
        unsafe {
            deinit();
        }
    }
}
