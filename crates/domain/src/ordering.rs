use crate::error::DomainError;
use fractional_index::FractionalIndex;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};

/// Algorithm v1: fractional_index 2.0.2, lowercase terminated hex strings.
/// Lexical UTF-8 byte ordering is identical to SQLite COLLATE BINARY ordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SortKey(FractionalIndex);
impl SortKey {
    pub fn between(left: Option<&Self>, right: Option<&Self>) -> Result<Self, DomainError> {
        match (left, right) {
            (Some(left), Some(right)) => {
                if left >= right {
                    return Err(DomainError::InvalidBounds);
                }
                // Upstream 2.0.2 subtracts one from the right byte before checking
                // equality. Strip common prefixes (especially 0x00) to prevent
                // underflow while retaining exactly the same key representation.
                let l = left.0.as_bytes();
                let r = right.0.as_bytes();
                let common = l
                    .iter()
                    .zip(r)
                    .take(l.len().min(r.len()) - 1)
                    .take_while(|(a, b)| a == b)
                    .count();
                let lo = FractionalIndex::from_bytes(l[common..].to_vec())
                    .map_err(|_| DomainError::InvalidSortKey)?;
                let hi = FractionalIndex::from_bytes(r[common..].to_vec())
                    .map_err(|_| DomainError::InvalidSortKey)?;
                let suffix =
                    FractionalIndex::new_between(&lo, &hi).ok_or(DomainError::InvalidBounds)?;
                let mut bytes = l[..common].to_vec();
                bytes.extend_from_slice(suffix.as_bytes());
                FractionalIndex::from_bytes(bytes)
                    .map(Self)
                    .map_err(|_| DomainError::InvalidSortKey)
            }
            _ => FractionalIndex::new(left.map(|k| &k.0), right.map(|k| &k.0))
                .map(Self)
                .ok_or(DomainError::InvalidBounds),
        }
    }
    /// Dense, deterministic keys for the supplied canonical item order.
    /// Big-endian fixed-width ordinals followed by the algorithm-v1 terminator
    /// are valid fractional indexes and compare identically in Rust and SQLite.
    /// The caller commits all keys and one canonical reorder event atomically.
    pub fn rebalance(count: usize) -> Result<Vec<Self>, DomainError> {
        let mut out = Vec::new();
        out.try_reserve(count)
            .map_err(|_| DomainError::InvalidValue("sort_count"))?;
        if count == 1 {
            out.push(Self::default());
            return Ok(out);
        }
        let width = (usize::BITS - count.saturating_sub(1).leading_zeros()).div_ceil(8) as usize;
        for ordinal in 0..count {
            let bytes = ordinal.to_be_bytes();
            let mut encoded = bytes[bytes.len() - width..].to_vec();
            encoded.push(0x80);
            let key =
                FractionalIndex::from_bytes(encoded).map_err(|_| DomainError::InvalidSortKey)?;
            out.push(Self(key));
        }
        Ok(out)
    }
}
impl FromStr for SortKey {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Validate ASCII/even length first: the dependency decodes two-byte slices.
        if s.is_empty()
            || !s.len().is_multiple_of(2)
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DomainError::InvalidSortKey);
        }
        let key = FractionalIndex::from_string(s).map_err(|_| DomainError::InvalidSortKey)?;
        if key.to_string() != s {
            return Err(DomainError::InvalidSortKey);
        }
        Ok(Self(key))
    }
}
impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_string())
    }
}
impl Serialize for SortKey {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for SortKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}
