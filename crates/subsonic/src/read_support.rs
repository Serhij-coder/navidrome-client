//! Shared bounded normalization work and profile identity checks.
use crate::{ExchangeError, song::array};
use feishin_domain::*;
use serde_json::{Map, Value};
type Result<T> = std::result::Result<T, ExchangeError>;

// Count entities and supplied metadata rows across the complete nested response.
// This is a work/item budget, not an allocator or transport reception guarantee.
pub(super) struct Budget {
    pub(super) limit: usize,
    pub(super) used: usize,
}
impl Budget {
    pub(super) fn take(&mut self, count: usize) -> Result<()> {
        self.used = self
            .used
            .checked_add(count)
            .filter(|used| *used <= self.limit)
            .ok_or(ExchangeError::Context(DomainError::Capacity))?;
        Ok(())
    }
    pub(super) fn metadata(
        &mut self,
        obj: &Map<String, Value>,
        keys: &[&str],
        context: OperationContext<'_>,
    ) -> Result<()> {
        for key in keys {
            if let Some(values) = array(obj, key, context)? {
                self.take(values.len())?;
            }
        }
        Ok(())
    }
}

pub(super) fn validate_identity(server: &ServerId, entity: &str) -> Result<()> {
    if server.as_str().trim().is_empty() || server.as_str().len() > 128 || entity.is_empty() {
        Err(ExchangeError::Malformed)
    } else {
        Ok(())
    }
}
