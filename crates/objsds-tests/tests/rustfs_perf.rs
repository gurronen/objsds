use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use objsds::{Error as ObjsdsError, Objsds};
use objsds_queue::{Ack, Error as QueueError, QueueBuilder};
use objsds_store_s3::StoreError;
use objsds_tests::{ensure_rustfs_bucket, rustfs_enabled, rustfs_store};

const DEFAULT_WORKERS: usize = 8;
const DEFAULT_OPERATIONS_PER_WORKER: usize = 25;
const DEFAULT_QUEUE_ITEMS: usize = 100;
const MAX_TRANSIENT_RETRIES: usize = 5;
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(10);

#[derive(Debug)]
struct OperationOutcome {
    kind: OutcomeKind,
    transient_responses: usize,
}

#[derive(Debug)]
enum OutcomeKind {
    Success,
    Conflict,
    TransientFailure,
}

type OperationResult = Result<OperationOutcome, String>;

#[derive(Debug)]
struct Measurements {
    attempts: usize,
    successes: usize,
    conflicts: usize,
    transient_failures: usize,
    transient_responses: usize,
    elapsed: Duration,
}

impl Measurements {
    fn print(&self, structure: &str, workers: usize, operations_per_worker: usize) {
        let seconds = self.elapsed.as_secs_f64();
        let attempt_rate = self.attempts as f64 / seconds;
        let success_rate = self.successes as f64 / seconds;
        let conflict_rate = self.conflicts as f64 / seconds;
        let transient_failure_rate = self.transient_failures as f64 / seconds;
        let conflict_percent = self.conflicts as f64 * 100.0 / self.attempts as f64;
        let transient_failure_percent =
            self.transient_failures as f64 * 100.0 / self.attempts as f64;
        println!(
            "objsds_cas_perf structure={structure} workers={workers} operations_per_worker={operations_per_worker} attempts={} successes={} conflicts={} transient_failures={} transient_responses={} elapsed_ms={} attempt_ops_per_sec={attempt_rate:.2} success_ops_per_sec={success_rate:.2} conflict_ops_per_sec={conflict_rate:.2} transient_failure_ops_per_sec={transient_failure_rate:.2} conflict_percent={conflict_percent:.2} transient_failure_percent={transient_failure_percent:.2}",
            self.attempts,
            self.successes,
            self.conflicts,
            self.transient_failures,
            self.transient_responses,
            self.elapsed.as_millis(),
        );
    }
}

#[test]
#[ignore = "opt-in RustFS contention performance evaluation"]
fn rustfs_log_and_map_cas_contention() -> Result<(), Box<dyn Error>> {
    if !rustfs_enabled() {
        return Err("set OBJSDS_RUSTFS_E2E=1 and start RustFS before running this test".into());
    }

    let workers = positive_env("OBJSDS_PERF_WORKERS", DEFAULT_WORKERS)?;
    let operations_per_worker = positive_env(
        "OBJSDS_PERF_OPERATIONS_PER_WORKER",
        DEFAULT_OPERATIONS_PER_WORKER,
    )?;

    ensure_rustfs_bucket()?;
    let namespace = format!(
        "rustfs-perf-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let client = Objsds::builder()
        .store(rustfs_store()?)
        .namespace(namespace)
        .build()?;

    let map = Arc::new(
        client
            .map::<usize>("contention-map")
            .schema("usize-v1")
            .create()?,
    );
    let map_measurements = run_contention(workers, operations_per_worker, {
        let map = Arc::clone(&map);
        move |worker, operation| {
            classify_with_retries(|| {
                map.insert(format!("worker-{worker}-operation-{operation}"), operation)
            })
        }
    })?;
    map_measurements.print("map", workers, operations_per_worker);

    let log = Arc::new(
        client
            .log::<usize>("contention-log")
            .schema("usize-v1")
            .create()?,
    );
    let log_measurements = run_contention(workers, operations_per_worker, {
        let log = Arc::clone(&log);
        move |_, operation| classify_with_retries(|| log.append(operation))
    })?;
    log_measurements.print("log", workers, operations_per_worker);

    Ok(())
}

#[test]
#[ignore = "opt-in RustFS queue work-item throughput evaluation"]
fn rustfs_queue_work_item_throughput() -> Result<(), Box<dyn Error>> {
    if !rustfs_enabled() {
        return Err("set OBJSDS_RUSTFS_E2E=1 and start RustFS before running this test".into());
    }

    let clients = positive_env("OBJSDS_QUEUE_PERF_CLIENTS", DEFAULT_WORKERS)?;
    let items = positive_env("OBJSDS_QUEUE_PERF_ITEMS", DEFAULT_QUEUE_ITEMS)?;
    ensure_rustfs_bucket()?;
    let namespace = format!(
        "rustfs-queue-perf-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let queue = Arc::new(
        QueueBuilder::<_, usize>::new(rustfs_store()?, namespace, "work-items")
            .schema("usize-v1")
            .create()?,
    );
    for item in 0..items {
        queue.publish(item)?;
    }

    let barrier = Arc::new(Barrier::new(clients + 1));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(clients);
    for _ in 0..clients {
        let barrier = Arc::clone(&barrier);
        let queue = Arc::clone(&queue);
        let completed = Arc::clone(&completed);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut conflicts = 0;
            let mut transient_responses = 0;
            while completed.load(Ordering::Acquire) < items {
                let claim = loop {
                    match queue.claim(Duration::from_secs(300)) {
                        Ok(claim) => break claim,
                        Err(QueueError::Conflict { .. }) => conflicts += 1,
                        Err(error) if is_transient_queue(&error) => {
                            transient_responses += 1;
                            thread::sleep(INITIAL_RETRY_DELAY);
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                };
                let Some(claim) = claim else {
                    thread::yield_now();
                    continue;
                };
                loop {
                    match queue.ack(claim.id, claim.lease_token) {
                        Ok(Ack::Acknowledged) => {
                            completed.fetch_add(1, Ordering::AcqRel);
                            break;
                        }
                        Ok(outcome) => {
                            return Err(format!("unexpected acknowledgement outcome: {outcome:?}"));
                        }
                        Err(QueueError::Conflict { .. }) => conflicts += 1,
                        Err(error) if is_transient_queue(&error) => {
                            transient_responses += 1;
                            thread::sleep(INITIAL_RETRY_DELAY);
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                }
            }
            Ok::<_, String>((conflicts, transient_responses))
        }));
    }

    let started = Instant::now();
    barrier.wait();
    let mut conflicts = 0;
    let mut transient_responses = 0;
    for handle in handles {
        let (worker_conflicts, worker_transient_responses) = handle
            .join()
            .map_err(|_| "queue throughput worker panicked")?
            .map_err(|error| format!("queue throughput worker failed: {error}"))?;
        conflicts += worker_conflicts;
        transient_responses += worker_transient_responses;
    }
    let elapsed = started.elapsed();
    let work_items_per_sec = items as f64 / elapsed.as_secs_f64();
    println!(
        "objsds_queue_perf clients={clients} items={items} completed={} elapsed_ms={} work_items_per_sec={work_items_per_sec:.2} cas_conflicts={conflicts} transient_responses={transient_responses}",
        completed.load(Ordering::Acquire),
        elapsed.as_millis(),
    );
    assert_eq!(completed.load(Ordering::Acquire), items);
    assert!(queue.is_empty()?);
    Ok(())
}

fn run_contention<F>(
    workers: usize,
    operations_per_worker: usize,
    operation: F,
) -> Result<Measurements, Box<dyn Error>>
where
    F: Fn(usize, usize) -> OperationResult + Send + Sync + 'static,
{
    let barrier = Arc::new(Barrier::new(workers + 1));
    let operation = Arc::new(operation);
    let mut handles = Vec::with_capacity(workers);

    for worker in 0..workers {
        let barrier = Arc::clone(&barrier);
        let operation = Arc::clone(&operation);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let mut successes = 0;
            let mut conflicts = 0;
            let mut transient_failures = 0;
            let mut transient_responses = 0;
            for index in 0..operations_per_worker {
                let outcome = operation(worker, index)?;
                transient_responses += outcome.transient_responses;
                match outcome.kind {
                    OutcomeKind::Success => successes += 1,
                    OutcomeKind::Conflict => conflicts += 1,
                    OutcomeKind::TransientFailure => transient_failures += 1,
                }
            }
            Ok::<_, String>((
                successes,
                conflicts,
                transient_failures,
                transient_responses,
            ))
        }));
    }

    let started = Instant::now();
    barrier.wait();
    let mut successes = 0;
    let mut conflicts = 0;
    let mut transient_failures = 0;
    let mut transient_responses = 0;
    for handle in handles {
        let (
            worker_successes,
            worker_conflicts,
            worker_transient_failures,
            worker_transient_responses,
        ) = handle
            .join()
            .map_err(|_| "contention worker panicked")?
            .map_err(|error| format!("contention worker failed: {error}"))?;
        successes += worker_successes;
        conflicts += worker_conflicts;
        transient_failures += worker_transient_failures;
        transient_responses += worker_transient_responses;
    }

    Ok(Measurements {
        attempts: workers * operations_per_worker,
        successes,
        conflicts,
        transient_failures,
        transient_responses,
        elapsed: started.elapsed(),
    })
}

fn classify_with_retries<T, F>(mut operation: F) -> OperationResult
where
    F: FnMut() -> Result<T, ObjsdsError<StoreError>>,
{
    let mut transient_responses = 0;
    let mut delay = INITIAL_RETRY_DELAY;
    loop {
        match operation() {
            Ok(_) => {
                return Ok(OperationOutcome {
                    kind: OutcomeKind::Success,
                    transient_responses,
                });
            }
            Err(ObjsdsError::Conflict(_)) => {
                return Ok(OperationOutcome {
                    kind: OutcomeKind::Conflict,
                    transient_responses,
                });
            }
            Err(error) if is_transient(&error) => {
                transient_responses += 1;
                if transient_responses > MAX_TRANSIENT_RETRIES {
                    return Ok(OperationOutcome {
                        kind: OutcomeKind::TransientFailure,
                        transient_responses,
                    });
                }
                thread::sleep(delay);
                delay *= 2;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn is_transient(error: &ObjsdsError<StoreError>) -> bool {
    let ObjsdsError::Store(error) = error else {
        return false;
    };
    let StoreError::Transport(error) = &error.source else {
        return false;
    };
    error
        .status()
        .is_some_and(|status| matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504))
}

fn is_transient_queue(error: &QueueError<StoreError>) -> bool {
    let QueueError::Store(StoreError::Transport(error)) = error else {
        return false;
    };
    error
        .status()
        .is_some_and(|status| matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504))
}

fn positive_env(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(default);
    };
    let value = value
        .into_string()
        .map_err(|_| format!("{name} must contain valid UTF-8"))?;
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid {name}={value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be greater than zero").into());
    }
    Ok(parsed)
}
