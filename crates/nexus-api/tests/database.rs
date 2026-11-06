//! Real PostgreSQL tests. Each sqlx::test creates and removes its own database.
use nexus_protocol::{Batch, EventInput};
use sqlx::{PgPool, Row};
use uuid::Uuid;
fn input() -> EventInput {
    EventInput {
        id: Uuid::new_v4(),
        source: "tests".into(),
        kind: "order.created".into(),
        payload: serde_json::json!({"amount":42}),
    }
}
async fn batch(pool: &PgPool, e: EventInput) -> nexus_protocol::Receipt {
    nexus_store::ingest(pool, &Batch { events: vec![e] })
        .await
        .unwrap()
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn retry_is_idempotent(pool: PgPool) {
    let e = input();
    assert_eq!(batch(&pool, e.clone()).await.accepted, 1);
    assert_eq!(batch(&pool, e).await.duplicates, 1);
    assert_eq!(nexus_store::stats(&pool).await.unwrap()["total"], 1);
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn conflict_rolls_back_entire_batch(pool: PgPool) {
    let mut e = input();
    batch(&pool, e.clone()).await;
    e.payload = serde_json::json!({"changed":true});
    let fresh = input();
    assert!(matches!(
        nexus_store::ingest(
            &pool,
            &Batch {
                events: vec![fresh.clone(), e]
            }
        )
        .await,
        Err(nexus_store::StoreError::Conflict)
    ));
    assert!(nexus_store::get(&pool, fresh.id).await.unwrap().is_none());
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn concurrent_retry_accepts_once(pool: PgPool) {
    let e = input();
    let (a, b) = tokio::join!(batch(&pool, e.clone()), batch(&pool, e));
    assert_eq!(a.accepted + b.accepted, 1);
    assert_eq!(a.duplicates + b.duplicates, 1);
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn worker_archives_and_acknowledges(pool: PgPool) {
    let e = input();
    batch(&pool, e.clone()).await;
    assert!(nexus_store::process_one(&pool).await.unwrap());
    assert!(!nexus_store::process_one(&pool).await.unwrap());
    let bytes: Vec<u8> = sqlx::query("SELECT encoded FROM archive WHERE event_id=$1")
        .bind(e.id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("encoded");
    assert_eq!(nexus_protocol::decode_event(&bytes).unwrap(), e);
    assert_eq!(nexus_store::stats(&pool).await.unwrap()["done"], 1);
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn workers_do_not_double_process(pool: PgPool) {
    for _ in 0..8 {
        batch(&pool, input()).await;
    }
    for _ in 0..4 {
        let (a, b) = tokio::join!(
            nexus_store::process_one(&pool),
            nexus_store::process_one(&pool)
        );
        assert!(a.unwrap());
        assert!(b.unwrap());
    }
    assert_eq!(nexus_store::stats(&pool).await.unwrap()["done"], 8);
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn cursor_and_filters(pool: PgPool) {
    for _ in 0..4 {
        batch(&pool, input()).await;
    }
    let a = nexus_store::list(&pool, i64::MAX, 2, Some("tests"), None)
        .await
        .unwrap();
    let b = nexus_store::list(&pool, a[1].0, 2, None, None)
        .await
        .unwrap();
    assert_eq!(b.len(), 2);
    assert!(a
        .iter()
        .all(|x| b.iter().all(|y| x.1.input.id != y.1.input.id)));
    assert!(nexus_store::list(&pool, i64::MAX, 10, Some("absent"), None)
        .await
        .unwrap()
        .is_empty());
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn failure_backoff_dead_letter_and_replay(pool: PgPool) {
    let e = input();
    batch(&pool, e.clone()).await;
    sqlx::query("ALTER TABLE archive ADD CONSTRAINT simulated_failure CHECK (false)")
        .execute(&pool)
        .await
        .unwrap();
    for _ in 0..5 {
        assert!(nexus_store::process_one(&pool).await.unwrap());
        sqlx::query("UPDATE deliveries SET available_at=now() WHERE event_id=$1")
            .bind(e.id)
            .execute(&pool)
            .await
            .unwrap();
    }
    assert_eq!(nexus_store::stats(&pool).await.unwrap()["dead"], 1);
    assert!(nexus_store::replay(&pool, e.id).await.unwrap());
    sqlx::query("ALTER TABLE archive DROP CONSTRAINT simulated_failure")
        .execute(&pool)
        .await
        .unwrap();
    assert!(nexus_store::process_one(&pool).await.unwrap());
    assert_eq!(nexus_store::stats(&pool).await.unwrap()["done"], 1);
    assert!(!nexus_store::replay(&pool, e.id).await.unwrap());
}
#[sqlx::test(migrations = "../../migrations")]
#[ignore = "requires PostgreSQL; run with --ignored"]
async fn aborted_claim_is_recoverable(pool: PgPool) {
    let e = input();
    batch(&pool, e).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT * FROM deliveries FOR UPDATE")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert!(!nexus_store::process_one(&pool).await.unwrap());
    tx.rollback().await.unwrap();
    assert!(nexus_store::process_one(&pool).await.unwrap());
}
