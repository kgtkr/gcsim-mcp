use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub struct SimLimiter {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl SimLimiter {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    pub fn try_acquire(&self) -> Result<OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().map_err(|_| {
            anyhow!(
                "Server busy: Maximum concurrent simulations ({}) reached. Please retry later.",
                self.max_concurrent
            )
        })
    }
}
