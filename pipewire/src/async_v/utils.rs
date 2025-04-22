//! Utility types for async bindings

use futures::task::{Context, Poll};
use futures::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Triple Modular Redundancy for radiation hardening
#[derive(Debug)]
pub struct TMR<T: PartialEq + Clone> {
    values: [Option<T>; 3],
}

impl<T: PartialEq + Clone> TMR<T> {
    /// Create a new TMR container with no values
    pub fn new() -> Self {
        Self {
            values: [None, None, None],
        }
    }

    /// Set all three redundant values
    pub fn set(&mut self, value: T) {
        for slot in &mut self.values {
            *slot = Some(value.clone());
        }
    }

    /// Get the value using majority voting to detect SEUs
    pub fn get(&self) -> Option<T> {
        match (&self.values[0], &self.values[1], &self.values[2]) {
            (Some(a), Some(b), Some(c)) if a == b || a == c => self.values[0].clone(),
            (Some(_), Some(b), Some(c)) if b == c => self.values[1].clone(),
            _ => None, // No consensus - possible radiation-induced error
        }
    }
}

/// Bounded queue with predictable memory usage
pub struct BoundedQueue<T, const N: usize> {
    items: [Option<T>; N],
    head: usize,
    tail: usize,
    len: usize,
}

impl<T, const N: usize> BoundedQueue<T, N> {
    pub fn new() -> Self {
        Self {
            items: std::array::from_fn(|_| None),
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.len == N {
            return Err(item);
        }

        self.items[self.tail] = Some(item);
        self.tail = (self.tail + 1) % N;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        let item = self.items[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        item
    }
}

/// Error type for timeout operations
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Deadline exceeded")]
    DeadlineExceeded,
    #[error("Maximum poll count exceeded")]
    MaxPollsExceeded,
}

/// Future with timeout and bounded execution guarantees
pub struct TimeoutFuture<F> {
    inner: F,
    deadline: Instant,
    poll_count: AtomicUsize,
    max_polls: usize,
}

impl<F: Future> TimeoutFuture<F> {
    pub fn new(future: F, timeout: Duration, max_polls: usize) -> Self {
        Self {
            inner: future,
            deadline: Instant::now() + timeout,
            poll_count: AtomicUsize::new(0),
            max_polls,
        }
    }
}

impl<F: Future> Future for TimeoutFuture<F> {
    type Output = Result<F::Output, TimeoutError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Check for poll count bound to prevent unbounded execution
        let current_count = self.poll_count.load(Ordering::SeqCst);
        if current_count >= self.max_polls {
            return Poll::Ready(Err(TimeoutError::MaxPollsExceeded));
        }

        // Check for time bound
        if Instant::now() > self.deadline {
            return Poll::Ready(Err(TimeoutError::DeadlineExceeded));
        }

        // Increment the poll count
        self.poll_count.fetch_add(1, Ordering::SeqCst);

        // Extract the inner future using proper Pin projection
        let inner = unsafe {
            let this = self.get_ref();
            Pin::new_unchecked(&mut *(&this.inner as *const F as *mut F))
        };

        // Poll the inner future
        match inner.poll(cx) {
            Poll::Ready(value) => Poll::Ready(Ok(value)),
            Poll::Pending => Poll::Pending,
        }
    }
}
