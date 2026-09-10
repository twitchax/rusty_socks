use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use tokio::sync::{Mutex, MutexGuard};

/// Number of buffers the pool retains for reuse.
///
/// Past this, [`BufferPool::lease`] still succeeds, but hands out a detached buffer whose memory is
/// released when it drops. Without a cap the pool only ever grows, so a single connection spike
/// pins `peak * buffer_size` bytes for the life of the process.
const MAX_POOLED_BUFFERS: usize = 512;

/// Indices of pooled buffers that are free to hand out.
///
/// A `std` mutex rather than a `tokio` one because [`Buffer::drop`] cannot await, and the critical
/// section is a single push or pop that never spans a yield point.
type FreeList = Arc<StdMutex<Vec<usize>>>;

pub struct BufferPool {
    buffer_size: usize,
    buffers: Vec<Arc<Mutex<Vec<u8>>>>,
    free: FreeList,
}

impl BufferPool {
    pub fn new(buffer_size: usize) -> Self {
        BufferPool {
            buffer_size,
            buffers: Vec::new(),
            free: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    /// Hand out a buffer in constant time.
    ///
    /// Reuses a freed buffer if one is available, otherwise grows the pool up to
    /// [`MAX_POOLED_BUFFERS`], and past that returns a detached buffer. This runs on the accept
    /// loop, so it must not scan: the previous implementation walked every buffer looking for an
    /// unleased one, which is `O(n)` per accept with `n` rising under load.
    pub fn lease(&mut self) -> Buffer {
        if let Some(index) = self.pop_free() {
            return Buffer::pooled(self.buffers[index].clone(), index, self.free.clone());
        }

        if self.buffers.len() < MAX_POOLED_BUFFERS {
            let index = self.add_buffer();
            return Buffer::pooled(self.buffers[index].clone(), index, self.free.clone());
        }

        Buffer::detached(self.allocate())
    }

    pub fn leased_count(&self) -> usize {
        self.buffers.iter().filter(|b| Arc::strong_count(b) >= 2).count()
    }

    pub fn total_count(&self) -> usize {
        self.buffers.len()
    }

    fn pop_free(&self) -> Option<usize> {
        self.free.lock().unwrap_or_else(PoisonError::into_inner).pop()
    }

    fn allocate(&self) -> Arc<Mutex<Vec<u8>>> {
        Arc::new(Mutex::new(vec![0; self.buffer_size]))
    }

    fn add_buffer(&mut self) -> usize {
        self.buffers.push(self.allocate());

        self.buffers.len() - 1
    }
}

pub struct Buffer {
    buffer: Arc<Mutex<Vec<u8>>>,
    /// Pool slot to return on drop. `None` for a detached buffer, which simply frees.
    index: Option<usize>,
    free: Option<FreeList>,
}

impl Buffer {
    fn pooled(buffer: Arc<Mutex<Vec<u8>>>, index: usize, free: FreeList) -> Buffer {
        Buffer {
            buffer,
            index: Some(index),
            free: Some(free),
        }
    }

    fn detached(buffer: Arc<Mutex<Vec<u8>>>) -> Buffer {
        Buffer { buffer, index: None, free: None }
    }

    pub async fn get(&mut self) -> MutexGuard<'_, Vec<u8>> {
        self.buffer.lock().await
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let (Some(index), Some(free)) = (self.index, self.free.as_ref()) {
            free.lock().unwrap_or_else(PoisonError::into_inner).push(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn lease_reuses_freed_buffers() {
        let mut pool = BufferPool::new(1024);
        assert_eq!(pool.total_count(), 0);

        let a = pool.lease();
        assert_eq!(pool.leased_count(), 1);
        assert_eq!(pool.total_count(), 1);

        // A second concurrent lease must allocate a new buffer.
        let b = pool.lease();
        assert_eq!(pool.total_count(), 2);

        // Dropping frees them; a subsequent lease reuses rather than growing the pool.
        drop(a);
        drop(b);
        assert_eq!(pool.leased_count(), 0);

        let _c = pool.lease();
        assert_eq!(pool.leased_count(), 1);
        assert_eq!(pool.total_count(), 2);
    }

    #[tokio::test]
    async fn leased_buffer_has_requested_size() {
        let mut pool = BufferPool::new(1500);
        let mut buffer = pool.lease();
        assert_eq!(buffer.get().await.len(), 1500);
    }

    #[test]
    fn dropping_a_buffer_returns_its_slot_to_the_free_list() {
        let mut pool = BufferPool::new(64);

        let a = pool.lease();
        assert_eq!(pool.free.lock().unwrap().len(), 0);

        drop(a);
        assert_eq!(pool.free.lock().unwrap().len(), 1);

        // Reusing the slot takes it back off the free list rather than allocating.
        let _b = pool.lease();
        assert_eq!(pool.free.lock().unwrap().len(), 0);
        assert_eq!(pool.total_count(), 1);
    }

    #[test]
    fn pool_stops_growing_at_the_cap_and_still_leases() {
        let mut pool = BufferPool::new(8);

        // Hold every pooled buffer at once so none can be reused.
        let held: Vec<Buffer> = (0..MAX_POOLED_BUFFERS).map(|_| pool.lease()).collect();
        assert_eq!(pool.total_count(), MAX_POOLED_BUFFERS);

        // The next lease succeeds without growing the pool: it is detached.
        let overflow = pool.lease();
        assert_eq!(pool.total_count(), MAX_POOLED_BUFFERS);

        // A detached buffer must not push a bogus slot back onto the free list.
        drop(overflow);
        assert_eq!(pool.free.lock().unwrap().len(), 0);

        drop(held);
        assert_eq!(pool.free.lock().unwrap().len(), MAX_POOLED_BUFFERS);
        assert_eq!(pool.leased_count(), 0);
    }

    #[tokio::test]
    async fn detached_buffer_has_requested_size() {
        let mut pool = BufferPool::new(128);
        let _held: Vec<Buffer> = (0..MAX_POOLED_BUFFERS).map(|_| pool.lease()).collect();

        let mut overflow = pool.lease();
        assert_eq!(overflow.get().await.len(), 128);
    }
}
