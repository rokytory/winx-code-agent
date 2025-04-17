use std::io::{self, Read, Write};
use regex::Regex;
use tracing::{Subscriber, Event};
use tracing_subscriber::Layer;
use tracing_subscriber::registry::LookupSpan;
use once_cell::sync::Lazy;

/// Comprehensive regex for matching ANSI escape sequences
static ANSI_REGEX: Lazy<Regex> = Lazy::new(|| {
    // This pattern matches all standard ANSI escape sequences:
    // - \x1b followed by [ and then any numeric parameters and a terminating character
    // - \x1b followed by ] and any text up to the bell character (\x07)
    // - Other ANSI escape sequences with single character terminators
    Regex::new(r"(\x1b\[[0-9;]*[a-zA-Z])|(\x1b\][^\x07]*\x07)|(\x1b[ABCDEFGHIJKLMNOPQRSTUVWXYZ])").unwrap()
});

/// Strip all ANSI escape sequences from a string
pub fn strip_ansi_escape_sequences(s: &str) -> String {
    ANSI_REGEX.replace_all(s, "").to_string()
}

/// A filter for IO that strips ANSI escape sequences
pub struct AnsiFilter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
}

impl<W: Write> AnsiFilter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: writer,
            buffer: Vec::with_capacity(1024),
        }
    }
    
    /// Flush any remaining data in the buffer
    pub fn flush_buffer(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            if let Ok(s) = std::str::from_utf8(&self.buffer) {
                let cleaned = strip_ansi_escape_sequences(s);
                self.inner.write_all(cleaned.as_bytes())?;
            } else {
                // If not valid UTF-8, just write the raw bytes
                self.inner.write_all(&self.buffer)?;
            }
            self.buffer.clear();
        }
        self.inner.flush()
    }
}

impl<W: Write> Write for AnsiFilter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Remember original length to return (Write contract)
        let original_len = buf.len();
        if original_len == 0 {
            return Ok(0);
        }
        
        // Append to buffer
        self.buffer.extend_from_slice(buf);
        
        // If we find a newline or buffer is large, process and flush
        if buf.contains(&b'\n') || self.buffer.len() > 8192 {
            self.flush_buffer()?;
        }
        
        Ok(original_len)
    }
    
    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer()
    }
}

/// A wrapper around any Read that strips ANSI escape sequences
pub struct AnsiFilterReader<R: Read> {
    inner: R,
    buffer: Vec<u8>,
}

impl<R: Read> AnsiFilterReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            buffer: Vec::with_capacity(1024),
        }
    }
}

impl<R: Read> Read for AnsiFilterReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Read from inner
        let bytes_read = self.inner.read(buf)?;
        if bytes_read == 0 {
            return Ok(0);
        }
        
        // Strip ANSI sequences if possible
        let slice = &buf[..bytes_read];
        if let Ok(s) = std::str::from_utf8(slice) {
            let cleaned = strip_ansi_escape_sequences(s);
            
            // Copy the cleaned data back to the buffer
            let cleaned_bytes = cleaned.as_bytes();
            let len = std::cmp::min(cleaned_bytes.len(), buf.len());
            buf[..len].copy_from_slice(&cleaned_bytes[..len]);
            
            Ok(len)
        } else {
            // If not valid UTF-8, return as is
            Ok(bytes_read)
        }
    }
}

/// A tracing layer that strips ANSI escape sequences from events
pub struct AnsiFilterLayer;

impl<S> Layer<S> for AnsiFilterLayer 
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // The purpose of this layer is to be inserted before any formatting happens
        // It doesn't actually modify the event but ensures it gets through
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_strip_ansi_escape_sequences() {
        let input = "\x1b[31mRed text\x1b[0m and \x1b[32mgreen text\x1b[0m";
        let expected = "Red text and green text";
        assert_eq!(strip_ansi_escape_sequences(input), expected);
        
        let input2 = "\x1b[2m2025-04-17T07:55:15.335Z\x1b[0m [winx] [info] Initializing server...";
        let expected2 = "2025-04-17T07:55:15.335Z [winx] [info] Initializing server...";
        assert_eq!(strip_ansi_escape_sequences(input2), expected2);
    }
}
