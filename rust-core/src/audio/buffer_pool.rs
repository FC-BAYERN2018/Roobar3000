use crate::audio::format::AudioFormat;
use ringbuf::{RingBuffer, Producer, Consumer};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct AudioBuffer {
    data: Vec<u8>,
    format: AudioFormat,
    frames: usize,
}

impl AudioBuffer {
    pub fn new(format: AudioFormat, frames: usize) -> Self {
        let size = format.bytes_per_frame() * frames;
        Self {
            data: vec![0u8; size],
            format,
            frames,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }
}

pub struct BufferPool {
    buffers: Vec<AudioBuffer>,
    free_buffers: Vec<usize>,
    format: AudioFormat,
    buffer_size_frames: usize,
}

impl BufferPool {
    pub fn new(format: AudioFormat, buffer_size_frames: usize, pool_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(pool_size);
        let mut free_buffers = Vec::with_capacity(pool_size);

        for _ in 0..pool_size {
            buffers.push(AudioBuffer::new(format, buffer_size_frames));
            free_buffers.push(buffers.len() - 1);
        }

        Self {
            buffers,
            free_buffers,
            format,
            buffer_size_frames,
        }
    }

    pub fn acquire(&mut self) -> Option<AudioBuffer> {
        if let Some(index) = self.free_buffers.pop() {
            let buffer = std::mem::replace(&mut self.buffers[index], AudioBuffer::new(self.format, 0));
            Some(buffer)
        } else {
            None
        }
    }

    pub fn release(&mut self, mut buffer: AudioBuffer) {
        if buffer.frames() == self.buffer_size_frames {
            buffer.data_mut().fill(0);
            self.buffers.push(buffer);
            self.free_buffers.push(self.buffers.len() - 1);
        }
    }

    pub fn available_count(&self) -> usize {
        self.free_buffers.len()
    }

    pub fn total_count(&self) -> usize {
        self.buffers.len()
    }
}

pub struct AudioRingBuffer {
    producer: Producer<u8>,
    consumer: Consumer<u8>,
    capacity: usize,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let rb = RingBuffer::new(capacity);
        let (producer, consumer) = rb.split();
        Self {
            producer,
            consumer,
            capacity,
        }
    }

    pub fn write(&mut self, data: &[u8]) -> usize {
        let written = self.producer.write_iter(data).count();
        written
    }

    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let read = self.consumer.read_iter(buf).count();
        read
    }

    pub fn available(&self) -> usize {
        self.producer.remaining()
    }

    pub fn len(&self) -> usize {
        self.consumer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.consumer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.producer.is_full()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        while !self.consumer.is_empty() {
            let _ = self.consumer.skip(1);
        }
    }
}

pub struct SharedRingBuffer {
    inner: Arc<Mutex<AudioRingBuffer>>,
}

impl SharedRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AudioRingBuffer::new(capacity))),
        }
    }

    pub fn write(&self, data: &[u8]) -> usize {
        self.inner.lock().write(data)
    }

    pub fn read(&self, buf: &mut [u8]) -> usize {
        self.inner.lock().read(buf)
    }

    pub fn available(&self) -> usize {
        self.inner.lock().available()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.inner.lock().is_full()
    }

    pub fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Clone for SharedRingBuffer {
    fn clone(&self) -> Self {
        self.clone()
    }
}
