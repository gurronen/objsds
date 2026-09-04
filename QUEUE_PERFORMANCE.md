# RustFS Queue work-item throughput report

This one-off local measurement was recorded on 2026-09-04. It characterizes the
single-object Queue protocol; it is not a provider-independent service-level
objective.

## Method

For each client count, the benchmark created a fresh Queue on RustFS and
published 100 `usize` work items before starting the timer. All clients then
contended on that one queue until every item had been claimed and acknowledged.
A completed work item is one successful claim-and-ack pair. The harness retries
explicit CAS conflicts and transient S3 responses; the public Queue API does
not retry. Leases were five minutes, so no measured item was intentionally
reclaimed.

The timed result excludes queue creation and publication. Runs used a debug
Rust test build, local Docker networking, and one run per client count.

## Results

| Clients | Items | Elapsed (ms) | Work items/s | CAS conflicts | Transient responses |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100 | 6,626 | 15.09 | 0 | 0 |
| 5 | 100 | 15,744 | 6.35 | 33 | 0 |
| 10 | 100 | 36,653 | 2.73 | 90 | 0 |
| 20 | 100 | 72,143 | 1.39 | 218 | 15 |
| 50 | 100 | 123,924 | 0.81 | 254 | 285 |

Throughput falls as clients increase because every claim and acknowledgement
rewrites the same object with compare-and-swap. The growing conflict count is
the expected serialization cost of the deliberately brokerless single-object
design. At 20 and 50 clients, transient responses also materially affected this
single run.

## Environment

- CPU: Intel Core i7-1185G7, 8 logical CPUs
- Memory: 31 GiB
- OS: Linux 7.1.8-arch1-3 x86_64
- Rust: 1.98.0
- RustFS container image: `rustfs/rustfs:latest`
- Image ID: `sha256:41fe89380f4120a337790c02af192c3fe7bb55c3edc2e6e9357b487b47c6ab21`

## Reproduce

Start RustFS, then run each client count with the same item count:

```console
pitchfork start rustfs
for clients in 1 5 10 20 50; do
  OBJSDS_RUSTFS_E2E=1 \
  OBJSDS_QUEUE_PERF_CLIENTS="$clients" \
  OBJSDS_QUEUE_PERF_ITEMS=100 \
    cargo test -p objsds-tests --features rustfs-e2e \
      --test rustfs_perf rustfs_queue_work_item_throughput \
      -- --ignored --nocapture
done
```

Results are sensitive to object size, payload size, build profile, client and
server hardware, network latency, RustFS version, and concurrent machine load.
Use repeated release-build runs for capacity planning.
