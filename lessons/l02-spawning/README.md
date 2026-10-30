# Lesson 02 — Tokio runtime + structured spawning

## Target

Learn to reach for a real runtime (Tokio) instead of your toy executor and understand the concurrency primitives it gives you at the boundary of `spawn`: task ownership, structured groups (`JoinSet`), bounded concurrency (`Semaphore`), and the meaning of `Send + 'static` on spawned futures.

**Observable competency.** You can (a) pick between `current_thread` and `multi_thread` runtimes and defend the choice for a given workload; (b) explain why `tokio::spawn` requires `Send + 'static` on its future by referencing the runtime's work-stealing scheduler; (c) demonstrate a deadlock caused by holding `std::sync::Mutex` across `.await` and fix it by shrinking the critical section; (d) show a bounded concurrency pattern using `JoinSet` + `Semaphore` where the max in-flight count is observably enforced.

## Prereqs

- L01 complete (custom executor + Future understanding).
- You understand: `Future::poll`, `Waker`, `Pin`, why `async fn` desugars to a state machine, why `'static` is required for spawned tasks.

## Materials

Read these in order. Mentor notes bridge canonical gaps.

1. **[Tokio tutorial](https://tokio.rs/tokio/tutorial)** — chapters *Hello Tokio*, *Spawning*, *Shared state*, *Channels*. The one idea to leave with: `#[tokio::main]` desugars to `Runtime::new().unwrap().block_on(async { … })` — nothing magic, just runtime setup + `block_on`.

2. **[`tokio::runtime::Builder`](https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html)** — read type-level docs. Focus on `new_current_thread` vs `new_multi_thread`, `worker_threads`, `enable_all` vs `enable_time` vs `enable_io`.

3. **[`tokio::task::JoinSet`](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html)** — read type-level docs + `spawn`, `join_next`, `abort_all` method docs.

4. **[`tokio::sync::Semaphore`](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html)** — read type-level docs + `acquire_owned` (this is the variant you want for spawned tasks).

5. **[Alice Ryhl — "Async: What is blocking?"](https://ryhl.io/blog/async-what-is-blocking/)** — canonical explainer for what breaks a runtime.

> **Offline note:** Run `cargo doc --workspace` before your flight so all of Tokio's type-level docs are cached locally at `target/doc/tokio/index.html`. `rustup doc --std` covers std.

### Mentor notes (gap bridges)

**Runtime flavors.** `current_thread` runs *all* tasks on one OS thread (the one that called `block_on`). Cheaper (no work-stealing overhead), simpler mental model (`Send` bound relaxed to `!Send` via `spawn_local` — but only inside a `LocalSet`), but tasks that block or CPU-hog will starve the entire runtime. `multi_thread` runs N worker threads (defaults to `num_cpus`) with work-stealing across them. Tasks may migrate between threads at any `.await` point, which is why the `Send + 'static` bound exists. Choose `current_thread` for a single-purpose worker (CLI tools, embedded-ish scripts); `multi_thread` for anything that needs to scale.

**Why `Send + 'static` on `tokio::spawn`.** `'static` because the task's lifetime is independent of the caller (spawn returns, task may still be running). `Send` because in `multi_thread`, the runtime may resume the task on a different worker thread than the one it was previously polled on. Both bounds are checked by the compiler; violations produce clear error messages.

**`std::sync::Mutex` across `.await` — the deadlock.** Even though `MutexGuard` is `Send` in modern std, holding one across an `.await` is still dangerous. The task might yield the thread while holding the lock; on `current_thread` a peer task on the same thread that needs the lock will block *forever* (the runtime cannot progress the first task to release the lock — it's the same thread). On `multi_thread` the situation is milder but still causes thread starvation. Rule: never hold `std::sync::Mutex` across `.await`. If you truly need to hold across await, use `tokio::sync::Mutex` — its `.lock().await` yields cooperatively.

**Clippy will catch this.** `clippy::await_holding_lock` fires on any guard held across await. Trust it — this is your automatic must-fix from L02 onward.

**`JoinSet` vs bare `JoinHandle`s.** `tokio::spawn` returns `JoinHandle<T>`. A `JoinHandle` dropped without `.await`ing it does NOT cancel the task — task keeps running (this is a common newcomer trap). `JoinSet` fixes this by giving you a container: `spawn` adds to the set, `join_next` polls for completions, dropping the set aborts all remaining tasks. Structured concurrency in a bag.

**`Semaphore::acquire_owned`.** Regular `.acquire()` returns a `SemaphorePermit<'a>` borrowing the semaphore. That borrow can't cross task boundaries. `acquire_owned` takes `Arc<Semaphore>` and returns `OwnedSemaphorePermit` — no lifetime, can be moved into a spawned task. Almost always what you want with `JoinSet`.

## Project assignment

**Path:** `lessons/l02-spawning/`
**Binary:** `worker-pool`

Build a bounded-concurrency worker pool that processes a stream of *simulated* jobs. No real network; everything runs offline using `tokio::time::sleep` to simulate work.

### Behavior spec

- CLI: `worker-pool --jobs <N> --concurrency <K> [--seed <S>]`
  - `--jobs`: total number of jobs to process (e.g., 20)
  - `--concurrency`: max in-flight jobs at any time (e.g., 4)
  - `--seed`: optional RNG seed for reproducibility (default: 42)
- Each job:
  - Has an id 0..N.
  - "Processes" by sleeping a pseudo-random duration in [100ms, 500ms] (deterministic given seed + job id — a simple hash like `(id.wrapping_mul(2654435761) ^ seed) % 400 + 100` millis is plenty; no `rand` crate needed).
  - Returns a `JobResult { id: u32, took: Duration, size: usize }` where `size` is another derived-from-id number.
- Print per completion: `job {id:>3} done in {took:?} (size={size})`.
- After all jobs complete, print a summary: total wall time, mean per-job time, throughput (jobs/sec).
- Observable concurrency bound: at NO point should more than K jobs be in `process` at once. Add a `tracing` (or `println!`) instrumentation that emits `in_flight={n}` whenever a job starts/finishes. Assert in a test that the max ever observed is ≤ K.

### Structural requirements

- Use `#[tokio::main]` with default flavor (`multi_thread`).
- Use `Arc<Semaphore>` + `acquire_owned` to enforce the concurrency bound.
- Use `JoinSet` to hold the spawned tasks. No orphan `JoinHandle`s — every spawn goes through `JoinSet::spawn`.
- Collect results by draining the set with `join_next`.
- Handle `Ctrl-C` by calling `set.abort_all()` and printing a partial summary. Use `tokio::signal::ctrl_c` in a `tokio::select!` alongside `set.join_next()`.

### Success criteria

- `cargo run -p l02-spawning -- --jobs 20 --concurrency 4` finishes cleanly, prints 20 completion lines + a summary.
- `cargo test -p l02-spawning` has at least:
  - `test_concurrency_bound_respected` — simulates jobs and asserts `in_flight` never exceeds the configured max.
  - `test_all_jobs_complete` — asserts the count of results equals `--jobs`.
  - `test_ctrl_c_aborts_cleanly` — drive the pool with a synthetic cancel token (or `tokio::time::pause` + a manual cancel signal), assert partial-count summary printed.
- Deterministic given `--seed`: two runs with same seed produce same *set* of completed results (order can vary due to scheduler jitter).
- No `unwrap` outside tests.
- `cargo clippy -- -D warnings` clean.

## Stretch goals

- **Retry with exponential backoff.** Fail 20% of jobs deterministically (`hash(id) % 5 == 0`); retry up to 3 times, waiting `100ms * 2^attempt`.
- **Per-"host" rate limit.** Group jobs by `id % 4` as pretend hosts; each host gets its own `Arc<Semaphore>` with a smaller cap. Two-level bounding (global + per-host).
- **Progress via `tokio::sync::watch`.** A background task publishes `(completed, in_flight)` to a `watch::Sender`; a printer task subscribes and re-renders a single terminal line at 10Hz.
- **Real HTTP (requires internet).** Swap the simulated `process` for `reqwest::get(url).await?.text().await?` on a list of URLs. Skip this on the plane; try when back online.

## Review tools

```bash
# Mentor baseline
cargo fmt --all -- --check
cargo clippy -p l02-spawning --all-targets -- -D warnings
cargo nextest run -p l02-spawning        # or cargo test if nextest not installed

# Async-specific
# clippy::await_holding_lock (part of default lints) — MUST NOT fire
cargo clippy -p l02-spawning -- -D clippy::await_holding_lock

# Structural
cargo tree -p l02-spawning               # confirm no cyclic or duplicate deps
cargo doc -p l02-spawning --no-deps      # doc surface

# Runtime diagnostics
RUST_LOG=trace cargo run -p l02-spawning -- --jobs 10 --concurrency 2
# Optional tokio-console taste:
# 1. add console-subscriber to dev-deps (already in workspace deps)
# 2. RUSTFLAGS="--cfg tokio_unstable" cargo run -p l02-spawning
# 3. in another terminal: tokio-console
```

### Tool primers (first-time introductions in this lesson)

- **`clippy::await_holding_lock`** — fires when a `MutexGuard` (std or tokio) is alive across an `.await`. Fix: shorten the critical section, drop the guard before await, or use `tokio::sync::Mutex::lock().await`. Automatic must-fix from L02 onward.
- **`cargo tree`** — shows the full dependency graph. Use it to see WHY you're pulling in some crate. `cargo tree -i <crate>` inverts: shows who depends on `<crate>`.
- **`tokio::signal::ctrl_c`** — an async fn that resolves when SIGINT is delivered. Use in a `select!` alongside the actual work to enable graceful cancel.

## Exit criteria

- [ ] All three required tests pass.
- [ ] `cargo clippy -- -D warnings` clean including `await_holding_lock`.
- [ ] `cargo fmt --check` clean.
- [ ] No `tokio::spawn` calls whose `JoinHandle` is dropped without a container (`JoinSet` or equivalent supervision).
- [ ] Concurrency bound is observably enforced (test proves it).
- [ ] Ctrl-C during a run produces a clean partial summary, no panics, no leaked tasks.
- [ ] You can verbalize:
  - Why `tokio::spawn` needs `Send + 'static`.
  - What deadlock a `std::sync::Mutex` across `.await` causes and why `tokio::sync::Mutex` does not.
  - Difference between `Semaphore::acquire` and `Semaphore::acquire_owned` and when you'd use each.

## What to ping when done

`review` — I run the tool sweep, walk `Cargo.toml` deps, read the pool driver top-to-bottom looking for orphan handles / long lock holds / missing cancellation paths. Two-bucket feedback. Clean review + explicit `next` → L03 (Async I/O + graceful shutdown, TCP echo server).

`stuck` — smallest hint. `give me the answer` — full solution.
