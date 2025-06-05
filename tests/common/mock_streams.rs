//! Mock stream implementations for testing wire protocol behavior
//!
//! This module provides various mock stream types that simulate different
//! network conditions and I/O patterns for comprehensive testing.

use std::io::Cursor;
use tokio::io::{AsyncRead, AsyncWrite};

/// Basic mock stream for simple read/write testing
pub struct MockStream {
    read_cursor: Cursor<Vec<u8>>,
    write_buffer: Vec<u8>,
}

impl MockStream {
    pub fn new() -> Self {
        Self {
            read_cursor: Cursor::new(Vec::new()),
            write_buffer: Vec::new(),
        }
    }

    pub fn with_data(data: Vec<u8>) -> Self {
        Self {
            read_cursor: Cursor::new(data),
            write_buffer: Vec::new(),
        }
    }

    pub fn get_written_data(&self) -> &[u8] {
        &self.write_buffer
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.read_cursor).poll_read(cx, buf)
    }
}

impl AsyncWrite for MockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write_buffer.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A controllable mock stream that returns predetermined read sizes to test partial I/O
pub struct ControlledMockStream {
    data: Vec<u8>,
    position: usize,
    read_sizes: Vec<usize>, // Predetermined sizes for each read operation
    read_count: usize,      // Track how many read operations have been performed
}

impl ControlledMockStream {
    /// Create a new ControlledMockStream with data and predetermined read sizes
    pub fn new(data: Vec<u8>, read_sizes: Vec<usize>) -> Self {
        Self {
            data,
            position: 0,
            read_sizes,
            read_count: 0,
        }
    }

    /// Check if all data has been read
    pub fn is_finished(&self) -> bool {
        self.position >= self.data.len()
    }
}

impl AsyncRead for ControlledMockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // If we've read all data, return 0 (EOF)
        if self.position >= self.data.len() {
            return std::task::Poll::Ready(Ok(()));
        }

        // Determine how many bytes to read this time
        let read_size = if self.read_count < self.read_sizes.len() {
            self.read_sizes[self.read_count]
        } else {
            // If we've exhausted predetermined sizes, read remaining data
            self.data.len() - self.position
        };

        // Calculate actual bytes to read (limited by available space and remaining data)
        let remaining_data = self.data.len() - self.position;
        let bytes_to_read =
            std::cmp::min(read_size, std::cmp::min(buf.remaining(), remaining_data));

        if bytes_to_read > 0 {
            // Copy data to the buffer
            let end_pos = self.position + bytes_to_read;
            buf.put_slice(&self.data[self.position..end_pos]);
            self.position = end_pos;
        }

        self.read_count += 1;
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ControlledMockStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // Not used for read testing
        std::task::Poll::Ready(Ok(0))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A mock stream that can simulate interrupted reads by providing data in specific chunks
/// This tests the protocol's ability to handle partial reads and resume correctly
pub struct InterruptibleMockStream {
    data: Vec<u8>,
    position: usize,
    chunk_sizes: Vec<usize>,    // Sizes of data chunks to return on each read
    read_count: usize,          // Track how many read operations have been performed
    total_interruptions: usize, // Track total interruptions for verification
}

impl InterruptibleMockStream {
    /// Create a new InterruptibleMockStream with data and predetermined chunk sizes
    pub fn new(data: Vec<u8>, interruption_points: Vec<usize>) -> Self {
        // Convert interruption points to chunk sizes
        let mut chunk_sizes = Vec::new();
        let mut last_pos = 0;

        for &interrupt_pos in &interruption_points {
            if interrupt_pos > last_pos {
                chunk_sizes.push(interrupt_pos - last_pos);
                last_pos = interrupt_pos;
            }
            // Add a very small chunk to simulate resumption after interruption
            chunk_sizes.push(1);
            last_pos += 1;
        }

        // Add final chunk for remaining data
        if last_pos < data.len() {
            chunk_sizes.push(data.len() - last_pos);
        }

        Self {
            data,
            position: 0,
            chunk_sizes,
            read_count: 0,
            total_interruptions: interruption_points.len(),
        }
    }

    /// Check if all data has been read
    pub fn is_finished(&self) -> bool {
        self.position >= self.data.len()
    }

    /// Get the number of interruptions that occurred
    pub fn interruption_count(&self) -> usize {
        self.total_interruptions
    }
}

impl AsyncRead for InterruptibleMockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // If we've read all data, return 0 (EOF)
        if self.position >= self.data.len() {
            return std::task::Poll::Ready(Ok(()));
        }

        // Determine how many bytes to read this time based on chunk sizes
        let chunk_size = if self.read_count < self.chunk_sizes.len() {
            self.chunk_sizes[self.read_count]
        } else {
            // If we've exhausted predetermined sizes, read remaining data
            self.data.len() - self.position
        };

        // Calculate actual bytes to read (limited by available space and remaining data)
        let remaining_data = self.data.len() - self.position;
        let bytes_to_read =
            std::cmp::min(chunk_size, std::cmp::min(buf.remaining(), remaining_data));

        if bytes_to_read > 0 {
            let end_pos = self.position + bytes_to_read;
            buf.put_slice(&self.data[self.position..end_pos]);
            self.position = end_pos;
        }

        self.read_count += 1;
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for InterruptibleMockStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // Not used for read testing
        std::task::Poll::Ready(Ok(0))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A controllable mock stream that accepts predetermined write sizes to test partial write operations
/// This simulates network backpressure and fragmented write scenarios
pub struct ControlledWriteMockStream {
    written_data: Vec<u8>,
    write_sizes: Vec<usize>, // Predetermined sizes for each write operation
    write_count: usize,      // Track how many write operations have been performed
}

impl ControlledWriteMockStream {
    /// Create a new ControlledWriteMockStream with predetermined write sizes
    pub fn new(write_sizes: Vec<usize>) -> Self {
        Self {
            written_data: Vec::new(),
            write_sizes,
            write_count: 0,
        }
    }

    /// Get all data written to the stream
    pub fn get_written_data(&self) -> &[u8] {
        &self.written_data
    }

    /// Get the number of write operations performed
    pub fn write_operation_count(&self) -> usize {
        self.write_count
    }
}

impl AsyncRead for ControlledWriteMockStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Not used for write testing
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ControlledWriteMockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // If we've exhausted predetermined write sizes, accept all remaining data
        if self.write_count >= self.write_sizes.len() {
            self.written_data.extend_from_slice(buf);
            return std::task::Poll::Ready(Ok(buf.len()));
        }

        // Determine how many bytes to accept this time
        let write_size = self.write_sizes[self.write_count];
        let bytes_to_write = std::cmp::min(write_size, buf.len());

        if bytes_to_write > 0 {
            // Accept only the predetermined number of bytes
            self.written_data.extend_from_slice(&buf[..bytes_to_write]);
        }

        self.write_count += 1;
        std::task::Poll::Ready(Ok(bytes_to_write))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// A mock stream that can simulate write buffer backpressure (`WouldBlock`) errors at specific points
/// This is used to test the protocol's ability to handle write interruptions and resume correctly
pub struct InterruptibleWriteMockStream {
    written_data: Vec<u8>,
    interruption_points: Vec<usize>, // Byte positions at which to simulate WouldBlock
    current_position: usize,         // Current write position in the stream
    write_count: usize,              // Track number of write operations
    interrupted_count: usize,        // Track how many interruptions occurred
}

impl InterruptibleWriteMockStream {
    /// Create a new InterruptibleWriteMockStream with specific interruption points
    ///
    /// # Arguments
    /// * `interruption_points` - Byte positions where WouldBlock errors should be simulated
    pub fn new(interruption_points: Vec<usize>) -> Self {
        Self {
            written_data: Vec::new(),
            interruption_points,
            current_position: 0,
            write_count: 0,
            interrupted_count: 0,
        }
    }

    /// Get the data that has been written to the stream
    pub fn get_written_data(&self) -> &[u8] {
        &self.written_data
    }

    /// Get the number of write operations performed
    pub fn write_operation_count(&self) -> usize {
        self.write_count
    }

    /// Get the number of interruptions that occurred
    pub fn interruption_count(&self) -> usize {
        self.interrupted_count
    }

    /// Check if we should interrupt at the current position
    fn should_interrupt(&self) -> bool {
        self.interruption_points.contains(&self.current_position)
    }
}

impl AsyncRead for InterruptibleWriteMockStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Not used for write testing, just return EOF
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for InterruptibleWriteMockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write_count += 1;

        // Check if we should simulate backpressure at this position
        if self.should_interrupt() {
            self.interrupted_count += 1;

            // Return WouldBlock to simulate write buffer backpressure
            return std::task::Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                format!(
                    "Simulated write buffer backpressure at position {}",
                    self.current_position
                ),
            )));
        }

        // Normal write operation - accept all the data
        self.written_data.extend_from_slice(buf);
        self.current_position += buf.len();

        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}
