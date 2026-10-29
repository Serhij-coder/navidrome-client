//! Bounded output for durable JSON. Allocator rounding and allocations performed
//! inside `Serialize` implementations are outside this output-buffer budget.
use serde::Serialize;
use std::io::{self, Write};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EncodeError {
    TooLarge,
    Invalid,
}

struct Sink {
    bytes: Vec<u8>,
    limit: usize,
    overflow: bool,
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let Some(end) = self.bytes.len().checked_add(buf.len()) else {
            self.overflow = true;
            return Err(io::Error::other("JSON output limit exceeded"));
        };
        if end > self.limit {
            self.overflow = true;
            return Err(io::Error::other("JSON output limit exceeded"));
        }
        if end > self.bytes.capacity() {
            let desired = self
                .bytes
                .capacity()
                .saturating_mul(2)
                .max(end)
                .min(self.limit);
            self.bytes.reserve_exact(desired - self.bytes.len());
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn encode<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, EncodeError> {
    let mut sink = Sink {
        bytes: Vec::new(),
        limit,
        overflow: false,
    };
    let result = serde_json::to_writer(&mut sink, value);
    if sink.overflow {
        return Err(EncodeError::TooLarge);
    }
    result.map_err(|_| EncodeError::Invalid)?;
    Ok(sink.bytes)
}

#[cfg(test)]
#[path = "../../../tests/storage/bounded_json.rs"]
mod tests;
