//! A way to stop a generation that is taking too long (a very large grid): the caller keeps a clone of the token and calls
//! [`Cancel::cancel`] from any thread; the generator looks at it in its long loops and between its phases and gives up with `None`.
//! Looking at it never changes what a generation that is not cancelled draws.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
