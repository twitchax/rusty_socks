use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

pub struct BufferPool {
    buffer_size: usize,
    buffers: Vec<Arc<Mutex<Vec<u8>>>>
}

impl BufferPool {
    pub fn new(buffer_size: usize) -> Self {
        BufferPool { buffer_size, buffers: Vec::<Arc<Mutex<Vec<u8>>>>::new() }
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
        if free_buffer_index == None {
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
    buffer: Arc<Mutex<Vec<u8>>>
}

impl Buffer {
    fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Buffer {
        Buffer { buffer }
    }

    pub async fn get(&mut self) -> MutexGuard<'_, Vec<u8>> {
        self.buffer.lock().await
    }
}