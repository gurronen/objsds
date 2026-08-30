use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use objsds::{Error as ObjsdsError, Objsds};
use objsds_store_s3::StoreError;
use objsds_tests::{ensure_rustfs_bucket, rustfs_enabled, rustfs_store};

const DEFAULT_WORKERS: usize = 8;
const DEFAULT_OPERATIONS_PER_WORKER: usize = 25;

type OperationResult = Result<bool, String>;

#[derive(Debug)]
struct Measurements {
    attempts: usize,
    successes: usize,
    conflicts: usize,
    elapsed: Duration,
}

impl Measurements {
    fn print(&self, structure: &str, workers: usize, operations_per_worker: usize) {
        let seconds = self.elapsed.as_secs_f64();
        let attempt_rate = self.attempts as f64 / seconds;
        let success_rate = self.successes as f64 / seconds;
        let conflict_rate = self.conflicts as f64 / seconds;
        let conflict_percent = self.conflicts as f64 * 100.0 / self.attempts as f64;
        println!(
            "objsds_cas_perf structure={structure} workers={workers} operations_per_worker={operations_per_worker} attempts={} successes={} conflicts={} elapsed_ms={} attempt_ops_per_sec={attempt_rate:.2} success_ops_per_sec={success_rate:.2} conflict_ops_per_sec={conflict_rate:.2} conflict_percent={conflict_percent:.2}",
            self.attempts,
            self.successes,
            self.conflicts,
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
            classify(map.insert(format!("worker-{worker}-operation-{operation}"), operation))
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
        move |_, operation| classify(log.append(operation))
    })?;
    log_measurements.print("log", workers, operations_per_worker);

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
            for index in 0..operations_per_worker {
                match operation(worker, index)? {
                    true => successes += 1,
                    false => conflicts += 1,
                }
            }
            Ok::<_, String>((successes, conflicts))
        }));
    }

    let started = Instant::now();
    barrier.wait();
    let mut successes = 0;
    let mut conflicts = 0;
    for handle in handles {
        let (worker_successes, worker_conflicts) = handle
            .join()
            .map_err(|_| "contention worker panicked")?
            .map_err(|error| format!("contention worker failed: {error}"))?;
        successes += worker_successes;
        conflicts += worker_conflicts;
    }

    Ok(Measurements {
        attempts: workers * operations_per_worker,
        successes,
        conflicts,
        elapsed: started.elapsed(),
    })
}

fn classify<T>(result: Result<T, ObjsdsError<StoreError>>) -> OperationResult {
    match result {
        Ok(_) => Ok(true),
        Err(ObjsdsError::Conflict(_)) => Ok(false),
        Err(error) => Err(error.to_string()),
    }
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
