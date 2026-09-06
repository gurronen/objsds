use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use objsds::{Error, Objsds};
use objsds_queue::{Ack, Clock, QueueBuilder};
use objsds_store::Location;
use objsds_tests::{assert_backend_contract, ensure_rustfs_bucket, rustfs_store, unique_namespace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct User {
    name: String,
}

#[derive(Clone)]
struct ManualClock(Arc<AtomicU64>);

impl Clock for ManualClock {
    fn now_millis(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[test]
fn full_rustfs_experience() -> Result<(), Box<dyn std::error::Error>> {
    ensure_rustfs_bucket()?;
    let store = rustfs_store()?;
    let namespace = unique_namespace("rustfs-e2e");
    let client = Objsds::builder()
        .store(store.clone())
        .namespace(&namespace)
        .build()?;

    let users = client.map::<User>("users").schema("user-v1").create()?;
    users.insert(
        "alice",
        User {
            name: "Alice".to_owned(),
        },
    )?;
    assert_eq!(
        users
            .get("alice")?
            .expect("inserted user should be present")
            .name,
        "Alice"
    );
    assert!(matches!(
        client.map::<User>("users").schema("user-v2").open(),
        Err(Error::Incompatible(_))
    ));

    let events = client
        .log::<String>("events")
        .schema("event-v1")
        .open_or_create()?;
    let first = events.append("created".to_owned())?;
    let second = events.append("updated".to_owned())?;
    assert!(first < second);
    assert_eq!(events.records_after(first)?[0].value, "updated");

    let queue = QueueBuilder::<_, String>::new(store.clone(), &namespace, "jobs")
        .schema("job-v1")
        .create()?;
    let message_id = queue.publish("index-user".to_owned())?;
    let first_claim = queue
        .claim(Duration::from_secs(30))?
        .expect("published message should be claimable");
    assert_eq!(first_claim.id, message_id);
    assert_eq!(first_claim.value, "index-user");
    assert_eq!(first_claim.attempt, 1);
    assert_eq!(
        queue.ack(first_claim.id, first_claim.lease_token)?,
        Ack::Acknowledged
    );
    assert!(queue.is_empty()?);

    let clock = ManualClock(Arc::new(AtomicU64::new(1_000)));
    let reclaim_queue = QueueBuilder::<_, String>::new(store.clone(), &namespace, "reclaims")
        .schema("job-v1")
        .clock(clock.clone())
        .create()?;
    let reclaimed_id = reclaim_queue.publish("retry-me".to_owned())?;
    let expired = reclaim_queue
        .claim(Duration::from_millis(100))?
        .expect("message should be initially claimable");
    clock.0.store(1_100, Ordering::SeqCst);
    let reclaimed = reclaim_queue
        .claim(Duration::from_millis(100))?
        .expect("expired lease should be reclaimable");
    assert_eq!(reclaimed.id, reclaimed_id);
    assert_eq!(reclaimed.attempt, 2);
    assert_ne!(reclaimed.lease_token, expired.lease_token);
    assert_eq!(
        reclaim_queue.ack(reclaimed.id, expired.lease_token)?,
        Ack::LeaseMismatch
    );

    assert_backend_contract(
        &store,
        Location::new(format!("{namespace}/contract/object.json"))?,
    );

    Ok(())
}
