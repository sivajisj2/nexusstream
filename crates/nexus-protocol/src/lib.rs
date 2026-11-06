//! Versioned wire protocol. No storage or HTTP dependencies.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_BATCH: usize = 100;
pub const MAX_PAYLOAD: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    /// Producer-generated ID. Reuse it when retrying the same event.
    pub id: Uuid,
    pub source: String,
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Event {
    #[serde(flatten)]
    pub input: EventInput,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub events: Vec<EventInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Receipt {
    pub accepted: usize,
    pub duplicates: usize,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("batch must contain 1..=100 events")]
    BatchSize,
    #[error(
        "source and kind must be 1..=128 ASCII letters, digits, dot, dash, underscore or slash"
    )]
    Name,
    #[error("payload must be a JSON object no larger than 64 KiB")]
    Payload,
    #[error("event IDs must be non-nil and unique inside a batch")]
    Id,
}

pub fn validate(batch: &Batch) -> Result<(), ValidationError> {
    if batch.events.is_empty() || batch.events.len() > MAX_BATCH {
        return Err(ValidationError::BatchSize);
    }
    let mut ids = std::collections::HashSet::new();
    for e in &batch.events {
        if e.id.is_nil() || !ids.insert(e.id) {
            return Err(ValidationError::Id);
        }
        for name in [&e.source, &e.kind] {
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
            {
                return Err(ValidationError::Name);
            }
        }
        if !e.payload.is_object()
            || serde_json::to_vec(&e.payload).map_or(true, |v| v.len() > MAX_PAYLOAD)
        {
            return Err(ValidationError::Payload);
        }
    }
    Ok(())
}

/// Canonical JSON transport representation, also used when a worker archives an event.
pub fn encode_event(event: &EventInput) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(event)
}
pub fn decode_event(bytes: &[u8]) -> Result<EventInput, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> EventInput {
        EventInput {
            id: Uuid::new_v4(),
            source: "billing".into(),
            kind: "invoice.created".into(),
            payload: serde_json::json!({"amount":42}),
        }
    }
    #[test]
    fn protocol_round_trip() {
        let e = event();
        assert_eq!(decode_event(&encode_event(&e).unwrap()).unwrap(), e);
    }
    #[test]
    fn valid_batch() {
        assert!(validate(&Batch {
            events: vec![event()]
        })
        .is_ok());
    }
    #[test]
    fn empty_batch() {
        assert_eq!(
            validate(&Batch { events: vec![] }),
            Err(ValidationError::BatchSize)
        );
    }
    #[test]
    fn oversized_batch() {
        assert_eq!(
            validate(&Batch {
                events: (0..101).map(|_| event()).collect()
            }),
            Err(ValidationError::BatchSize)
        );
    }
    #[test]
    fn duplicate_ids() {
        let e = event();
        assert_eq!(
            validate(&Batch {
                events: vec![e.clone(), e]
            }),
            Err(ValidationError::Id)
        );
    }
    #[test]
    fn invalid_names() {
        let mut e = event();
        e.source = "bad source".into();
        assert_eq!(
            validate(&Batch { events: vec![e] }),
            Err(ValidationError::Name)
        );
    }
    #[test]
    fn rejects_array_payload() {
        let mut e = event();
        e.payload = serde_json::json!([]);
        assert_eq!(
            validate(&Batch { events: vec![e] }),
            Err(ValidationError::Payload)
        );
    }
    #[test]
    fn rejects_large_payload() {
        let mut e = event();
        e.payload = serde_json::json!({"x":"x".repeat(MAX_PAYLOAD)});
        assert_eq!(
            validate(&Batch { events: vec![e] }),
            Err(ValidationError::Payload)
        );
    }
    #[test]
    fn rejects_unknown_fields() {
        assert!(decode_event(br#"{"id":"00000000-0000-0000-0000-000000000001","source":"a","kind":"b","payload":{},"secret":1}"#).is_err());
    }
}
