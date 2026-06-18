use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

pub struct BufferPool {
    buffer_size: usize,
    buffers: Vec<Arc<Mutex<Vec<u8>>>>,
}

impl BufferPool {
    pub fn new(buffer_size: usize) -> Self {
        BufferPool {
            buffer_size,
            buffers: Vec::<Arc<Mutex<Vec<u8>>>>::new(),
        }
    }

    pub fn lease(&mut self) -> Buffer {
        let mut free_buffer_index: Option<usize> = None;

        // Find an unleased buffer.
        for k in 0..self.buffers.len() {
            let ref_count = Arc::strong_count(&self.buffers[k]);
            if ref_count < 2 {
                free_buffer_index = Some(k);
                break;
            }
        }

        // Or, create a new one.
        if free_buffer_index.is_none() {
            free_buffer_index = Some(self.add_buffer());
        }

        assert_ne!(None, free_buffer_index);

        let index = free_buffer_index.unwrap();

        Buffer::new(self.buffers[index].clone())
    }

    pub fn leased_count(&self) -> usize {
        self.buffers.iter().filter(|b| Arc::strong_count(b) >= 2).count()
    }

    pub fn total_count(&self) -> usize {
        self.buffers.len()
    }

    fn add_buffer(&mut self) -> usize {
        self.buffers.push(Arc::new(Mutex::new(vec![0; self.buffer_size])));

        self.buffers.len() - 1
    }
}

pub struct Buffer {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Buffer {
    fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Buffer {
        Buffer { buffer }
    }

    pub async fn get(&mut self) -> MutexGuard<'_, Vec<u8>> {
        self.buffer.lock().await
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
}
