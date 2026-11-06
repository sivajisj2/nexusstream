//! PostgreSQL persistence, idempotency, cursor reads and transactional delivery.
use nexus_protocol::{Batch, Event, EventInput, Receipt};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("event ID already exists with different content")]
    Conflict,
    #[error(transparent)]
    Encoding(#[from] serde_json::Error),
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}

/// Atomic batch: event + delivery record become visible together. Concurrent
/// retries are serialized by the unique constraint; conflicting content rolls back.
pub async fn ingest(pool: &PgPool, batch: &Batch) -> Result<Receipt, StoreError> {
    let mut tx = pool.begin().await?;
    let mut result = Receipt {
        accepted: 0,
        duplicates: 0,
    };
    // Consistent lock ordering avoids deadlocks between overlapping batches.
    let mut events: Vec<_> = batch.events.iter().collect();
    events.sort_by_key(|e| e.id);
    for e in events {
        let inserted=sqlx::query("INSERT INTO events(id,source,kind,payload) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO NOTHING")
            .bind(e.id).bind(&e.source).bind(&e.kind).bind(&e.payload).execute(&mut *tx).await?.rows_affected();
        if inserted == 1 {
            sqlx::query("INSERT INTO deliveries(event_id) VALUES($1)")
                .bind(e.id)
                .execute(&mut *tx)
                .await?;
            result.accepted += 1;
        } else {
            let row = sqlx::query("SELECT source,kind,payload FROM events WHERE id=$1")
                .bind(e.id)
                .fetch_one(&mut *tx)
                .await?;
            if row.get::<String, _>("source") != e.source
                || row.get::<String, _>("kind") != e.kind
                || row.get::<serde_json::Value, _>("payload") != e.payload
            {
                return Err(StoreError::Conflict);
            }
            result.duplicates += 1;
        }
    }
    tx.commit().await?;
    Ok(result)
}

fn event(row: &sqlx::postgres::PgRow) -> Event {
    Event {
        input: EventInput {
            id: row.get("id"),
            source: row.get("source"),
            kind: row.get("kind"),
            payload: row.get("payload"),
        },
        received_at: row.get("received_at"),
    }
}

pub async fn list(
    pool: &PgPool,
    before: i64,
    limit: i64,
    source: Option<&str>,
    kind: Option<&str>,
) -> Result<Vec<(i64, Event)>, sqlx::Error> {
    let rows=sqlx::query("SELECT * FROM events WHERE seq<$1 AND ($3::text IS NULL OR source=$3) AND ($4::text IS NULL OR kind=$4) ORDER BY seq DESC LIMIT $2")
        .bind(before).bind(limit).bind(source).bind(kind).fetch_all(pool).await?;
    Ok(rows.iter().map(|r| (r.get("seq"), event(r))).collect())
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Event>, sqlx::Error> {
    Ok(sqlx::query("SELECT * FROM events WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .as_ref()
        .map(event))
}
pub async fn stats(pool: &PgPool) -> Result<serde_json::Value, sqlx::Error> {
    let row=sqlx::query("SELECT (SELECT count(*) FROM events) AS total, count(*) FILTER(WHERE state='pending') AS pending, count(*) FILTER(WHERE state='done') AS done, count(*) FILTER(WHERE state='dead') AS dead FROM deliveries").fetch_one(pool).await?;
    Ok(
        serde_json::json!({"total":row.get::<i64,_>("total"),"pending":row.get::<i64,_>("pending"),"done":row.get::<i64,_>("done"),"dead":row.get::<i64,_>("dead")}),
    )
}

/// Lock one due delivery until the archive and acknowledgement commit together.
/// A process crash rolls back the claim. Multiple instances safely skip locked rows.
pub async fn process_one(pool: &PgPool) -> Result<bool, StoreError> {
    let mut tx = pool.begin().await?;
    let row=sqlx::query("SELECT e.* FROM deliveries d JOIN events e ON e.id=d.event_id WHERE d.state='pending' AND d.available_at<=now() ORDER BY d.available_at FOR UPDATE OF d SKIP LOCKED LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(false) };
    let e = event(&row).input;
    let bytes = nexus_protocol::encode_event(&e)?;
    sqlx::query("SAVEPOINT sink").execute(&mut *tx).await?;
    match sqlx::query(
        "INSERT INTO archive(event_id,encoded) VALUES($1,$2) ON CONFLICT(event_id) DO NOTHING",
    )
    .bind(e.id)
    .bind(bytes)
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {
            sqlx::query("UPDATE deliveries SET state='done',attempts=attempts+1,last_error=NULL WHERE event_id=$1").bind(e.id).execute(&mut *tx).await?;
        }
        Err(_) => {
            sqlx::query("ROLLBACK TO SAVEPOINT sink")
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE deliveries SET attempts=attempts+1, state=CASE WHEN attempts+1>=5 THEN 'dead' ELSE 'pending' END, available_at=now()+make_interval(secs => power(2,least(attempts+1,10))::int), last_error='archive write failed' WHERE event_id=$1").bind(e.id).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(true)
}
pub async fn replay(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("UPDATE deliveries SET state='pending', attempts=0,available_at=now(),last_error=NULL WHERE event_id=$1 AND state='dead'").bind(id).execute(pool).await?.rows_affected()==1)
}
