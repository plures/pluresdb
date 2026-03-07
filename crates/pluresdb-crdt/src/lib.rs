//! Deterministic HAM-style conflict decision helpers.
//!
//! This crate provides a small, pure decision function that can be reused by
//! storage and replication layers when applying Hypothetical Amnesia Machine
//! (HAM) conflict resolution.
//!
//! ## Decision order
//!
//! 1. **Newer timestamp wins** (`incoming.timestamp > current.timestamp`).
//! 2. **Historical writes are rejected** (`incoming.timestamp < current.timestamp`).
//! 3. **Equal timestamps are tie-broken deterministically** with a stable,
//!    lexicographic comparison of caller-supplied value keys.
//!
//! The `value_key` should be a deterministic byte representation (for example,
//! canonical JSON bytes, a hash digest, or another stable encoding) to ensure
//! all peers choose the same winner.

/// A timestamped value reference used for HAM merge decisions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HamValueRef<'a> {
    /// HAM state timestamp (milliseconds since Unix epoch, typically).
    pub timestamp: f64,
    /// Deterministic value key used for equal-timestamp tie-breaks.
    pub value_key: &'a [u8],
}

/// Deterministic outcome of comparing current vs incoming state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HamDecision {
    /// Incoming value should replace current value.
    AcceptIncoming,
    /// Current value should remain unchanged.
    KeepCurrent,
}

/// Decide whether an incoming field value should overwrite the current value.
///
/// This function is deterministic for all peers when `value_key` is deterministic.
///
/// # Rules
///
/// - `incoming.timestamp > current.timestamp` => [`HamDecision::AcceptIncoming`]
/// - `incoming.timestamp < current.timestamp` => [`HamDecision::KeepCurrent`]
/// - equal timestamps => lexicographically larger `value_key` wins
pub fn decide_ham_merge(current: HamValueRef<'_>, incoming: HamValueRef<'_>) -> HamDecision {
    if incoming.timestamp > current.timestamp {
        return HamDecision::AcceptIncoming;
    }

    if incoming.timestamp < current.timestamp {
        return HamDecision::KeepCurrent;
    }

    if incoming.value_key > current.value_key {
        HamDecision::AcceptIncoming
    } else {
        HamDecision::KeepCurrent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_timestamp_wins() {
        let current = HamValueRef {
            timestamp: 1_700_000_000_000.0,
            value_key: b"alice",
        };
        let incoming = HamValueRef {
            timestamp: 1_700_000_000_123.0,
            value_key: b"bob",
        };

        assert_eq!(decide_ham_merge(current, incoming), HamDecision::AcceptIncoming);
    }

    #[test]
    fn equal_timestamp_uses_deterministic_tie_break() {
        let current = HamValueRef {
            timestamp: 1_700_000_000_000.0,
            value_key: b"alpha",
        };
        let incoming = HamValueRef {
            timestamp: 1_700_000_000_000.0,
            value_key: b"omega",
        };

        assert_eq!(decide_ham_merge(current, incoming), HamDecision::AcceptIncoming);
    }

    #[test]
    fn historical_update_is_rejected() {
        let current = HamValueRef {
            timestamp: 1_700_000_000_100.0,
            value_key: b"current",
        };
        let incoming = HamValueRef {
            timestamp: 1_700_000_000_050.0,
            value_key: b"incoming",
        };

        assert_eq!(decide_ham_merge(current, incoming), HamDecision::KeepCurrent);
    }

    #[test]
    fn equal_timestamp_equal_value_key_keeps_current() {
        let current = HamValueRef {
            timestamp: 42.0,
            value_key: b"same",
        };
        let incoming = HamValueRef {
            timestamp: 42.0,
            value_key: b"same",
        };

        assert_eq!(decide_ham_merge(current, incoming), HamDecision::KeepCurrent);
    }
}
