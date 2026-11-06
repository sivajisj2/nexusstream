CREATE TABLE events (
    seq BIGSERIAL UNIQUE NOT NULL,
    id UUID PRIMARY KEY,
    source VARCHAR(128) NOT NULL,
    kind VARCHAR(128) NOT NULL,
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload) = 'object'),
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX events_source_seq ON events(source, seq DESC);
CREATE INDEX events_kind_seq ON events(kind, seq DESC);
CREATE TABLE deliveries (
    event_id UUID PRIMARY KEY REFERENCES events(id),
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','done','dead')),
    attempts INTEGER NOT NULL DEFAULT 0,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT
);
CREATE INDEX deliveries_pending ON deliveries(available_at) WHERE state='pending';
-- A durable internal sink. Replace with a real destination adapter as needed.
CREATE TABLE archive (
    event_id UUID PRIMARY KEY REFERENCES events(id),
    encoded BYTEA NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
