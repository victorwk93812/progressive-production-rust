# Lesson 01 — Runtime mental model

## Target

Internalize that `async fn` is **syntactic sugar for a hand-written state machine**, and that an *executor* is just a loop that polls futures and a *waker* is just a callback that says "this future is worth polling again." After this lesson, you can sketch the state machine for any small `async fn` on paper and explain why a future stalls forever if its dependency forgets to wake it.

**Observable competency.** You can hand a stranger your toy executor source, point at any `.poll(cx)` call, and explain: (a) what `cx.waker()` is for, (b) what happens if `Poll::Pending` is returned without arming the waker, (c) why the executor’s "wake" callback does not run the future inline but instead pushes a task ID onto a queue.

## Prereqs

- Earlier lessons: none (course entry point).
- Assumed: you've written `async fn` and `.await` in Tokio code before. You know what a trait object is. You can read `std::task` doc pages.

## Materials

Read these in order. The mentor notes below bridge the gaps the canonical sources leave open.

1. **[Asynchronous Programming in Rust](https://rust-lang.github.io/async-book/) — Chapters 1, 2, 3.**
   Focus on the `SimpleFuture` walkthrough in Ch.2 and the manual `MainFuture` state machine in Ch.3. *The one idea to leave with:* `async fn` compiles to an enum whose variants are the points between two `.await`s.

2. **[`std::future::Future`](https://doc.rust-lang.org/std/future/trait.Future.html) and [`std::task`](https://doc.rust-lang.org/std/task/index.html) module docs.**
   Read the *long-form* module doc for `std::task` — it specifies the waker contract precisely. *The one idea to leave with:* `Waker` is `Clone + Send + Sync` because it crosses thread boundaries between the reactor (who arms it) and the executor (who runs the task it points to).

3. **Jon Gjengset — *The What and How of Futures and async/await in Rust* (YouTube).**
   Skim if pressed for time. *The one idea to leave with:* the pin requirement on `poll` exists because the compiler-generated state machine may contain *internal references* (a `&` to one of its own locals across `.await`).

### Mentor notes (gap bridges)

**Why `Poll::Pending` instead of "block here".** The future is not allowed to block the thread because the executor wants to keep polling *other* tasks on the same thread. So `Pending` means: "I have no progress to make right now. I have arranged for someone — the *reactor* — to call my waker when I might have progress." If you forget that second clause, your future is silently dead. The executor will never re-poll a task whose waker was never armed.

**What a *reactor* is.** Just the thing that owns the I/O readiness machinery (epoll / kqueue / IOCP) or the timer wheel. It maps "the OS told me fd 7 is readable" to "wake the task that's blocked on fd 7". For L01 there is no real reactor — your `Delay` future will spawn a tiny `std::thread::sleep`-based timer thread and call the waker from there. That toy reactor is enough to make the mental model concrete.

**The `Context` and `Waker` split.** `Context<'_>` is the parameter you actually receive in `poll`. It currently only exposes `.waker()`, but it exists so that future Rust versions can add more context (e.g. local-storage handles) without breaking the trait. Don't overthink it.

**"Cooperative" vs preemptive.** A future relinquishes the thread by returning `Pending`. If a future loops forever without `Pending`, it starves all other tasks on the same thread. This is *the* defining property of cooperative scheduling and the reason `tokio::task::yield_now()` exists.

## Project assignment

**Path:** `lessons/l01-runtime/`
**Binary:** `toy-executor`
**Constraints:** `std` only (plus optional `pin-project-lite` and `futures` for `BoxFuture` if you must). **No Tokio. No `async-std`. No `smol`.**

Build a *single-threaded* executor and exercise it with three hand-rolled futures.

### Required components

1. **`SpawnHandle` / `Executor`.**
   - An `Executor` owns a `VecDeque<TaskId>` ready queue and a `HashMap<TaskId, Task>` task store.
   - A `Task` holds `Pin<Box<dyn Future<Output = ()>>>` plus a flag indicating whether it's currently queued (to prevent double-queue when wakers fire twice).
   - `Executor::spawn(fut)` boxes the future, allocates a `TaskId`, inserts the task, and queues it as ready.
   - `Executor::run()` loops: pop a `TaskId` from the ready queue, build a `Waker` whose `wake` callback re-queues this `TaskId`, poll the future. If `Ready`, drop the task. If `Pending`, do *not* re-queue (the waker is the only path back into the queue). Exit when there are no live tasks left.

2. **Custom `Waker` via `RawWaker` / `RawWakerVTable` (or `std::task::Wake` if you prefer the safer API).**
   The wake side and the executor share state through `Arc<Mutex<...>>` (or `Arc<Inner>` with interior `Mutex`). The wake callback re-queues the task ID; it does **not** poll inline.

3. **Hand-written futures (no `async fn` allowed for these three).**
   - `Ready<T>` — completes immediately on first poll. (Trivial; gets the API right.)
   - `Yield` — returns `Pending` once after arming `cx.waker().wake_by_ref()`, then `Ready(())` on the next poll. (This is the manual version of `tokio::task::yield_now`.)
   - `Delay { until: Instant }` — on first poll, spawn a `std::thread` that sleeps until `until`, then calls the cloned waker; the future returns `Pending`. On the second poll, return `Ready(())`. (Be careful: poll may be called before the thread fires — that's fine. May also be called after — also fine. Both paths must work.)

4. **Composite future: `Join2<A, B>`.**
   Polls both children. Completes when both are `Ready`. May be implemented with `pin-project-lite` *or* with a manual `Pin::map_unchecked_mut` projection (only if you justify it in a comment).

5. **`main()` demo.** Use `async fn`s built on top of the primitives above so the output proves cooperative interleaving:

   ```text
   t=0.00  task A: start
   t=0.00  task B: start
   t=0.00  task A: yielded
   t=0.00  task B: yielded
   t=0.50  task A: delay done
   t=0.75  task B: delay done
   t=0.75  joined
   ```

### Behavior spec

- Two `Delay`s started at the same wall-clock instant must finish in chronological order of their deadlines, not in spawn order.
- `Yield` must produce visible interleaving between two tasks.
- The executor must terminate cleanly when all spawned tasks complete.
- No spinning: when all tasks are `Pending` and the ready queue is empty, the main thread must `park` (or block on a condvar) until a waker wakes something. Burning CPU in a busy loop is a must-fix.

### Success criteria

- Running `cargo run -p l01-runtime` produces output matching the cooperative-interleaving pattern (timestamps near-monotonic, A and B interleaved at the `Yield`).
- `cargo test -p l01-runtime` passes at least these tests:
  - `ready_resolves_in_one_poll`
  - `yield_is_pending_then_ready`
  - `delay_completes_after_deadline` (using a short delay, e.g. 50 ms)
  - `join2_completes_when_both_complete`
  - `executor_terminates_when_all_tasks_done`
- No `unsafe` outside the `RawWaker` vtable construction (which is the one place it's expected). If you go the `std::task::Wake` route, the crate stays `#![forbid(unsafe_code)]` clean.
- The `Cargo.toml` `unsafe_code` lint should be flipped to `allow` *only* in the module containing the vtable, if needed, with a doc-comment justifying it.

## Stretch goals

These are optional. Skip without guilt — must-fix list is what gates graduation.

- **`Select2<A, B>`** — completes with the *first* of the two; drops the loser. Think about cancellation safety while you do it.
- **`block_on(future)`** — equivalent to `Executor::new().spawn(future).run()`, but returns the spawned future's output. Requires a `oneshot`-style channel internally; you can implement that channel by hand too.
- **`spawn(future)` returning a `JoinHandle<T>` whose `.await` resolves to `T`.** Now your executor is two-thirds of the way to `tokio::task::JoinHandle`.
- **Print the generated state machine for one of your `async fn`s with `cargo expand --bin toy-executor`** and commit the output to `docs/expanded.rs` with annotations of which enum variant corresponds to which `.await`.

## Review tools

Mentor will run these on `review`. Run them yourself first to catch issues before submitting.

```bash
# Formatting + lint (mentor's universal baseline)
cargo fmt --all -- --check
cargo clippy -p l01-runtime --all-targets -- -D warnings

# Tests
cargo test  -p l01-runtime
# (or `cargo nextest run -p l01-runtime` if installed)

# Macro expansion — required for the stretch task, optional otherwise
cargo expand -p l01-runtime --bin toy-executor > /tmp/expanded.rs
# look at the enum the compiler generated for your async fn

# Doc surface (cheap, catches broken intra-doc links)
cargo doc -p l01-runtime --no-deps
```

### Tool primers (first-time)

- **`cargo clippy -- -D warnings`** — every warning is an error. Read the lint name, then the docs link clippy prints. The lints we care about most for this lesson: `clippy::await_holding_lock`, `clippy::large_futures`, `clippy::needless_pass_by_value`. (Most won't fire on L01 since there's no `await` in your hand-rolled futures, but they will on every subsequent lesson.)
- **`cargo expand`** — invokes the compiler with macro expansion enabled. Useful here to *see* the enum your `async fn` desugars to. Install: `cargo install cargo-expand`. Requires nightly toolchain for some macros (not for the desugaring we want — stable works).

## Exit criteria

- [ ] All five required tests pass.
- [ ] `cargo clippy -- -D warnings` clean (or with documented `#[allow]` per occurrence).
- [ ] `cargo fmt --check` clean.
- [ ] No busy-spin: when the executor has nothing ready, the main thread sleeps.
- [ ] No `unsafe` outside the vtable construction (or zero unsafe if you used `std::task::Wake`).
- [ ] You can verbalize, in your own words, the answer to: **"In my `Delay` future, what would happen if I returned `Pending` *before* spawning the timer thread?"** (Expected: nothing. The waker is never armed, so the task never re-enters the queue, and `Executor::run` will park forever or — if other tasks complete first — exit while this task is still alive but unreachable.)

## What to ping when done

`review` — I'll run the tool sweep, walk your `RawWaker` vtable / state types line-by-line, look for spin loops or double-queue bugs, and give two-bucket feedback. If clean, I emit L02 on your next `next` ping.

If you get stuck, ping `stuck` with what you tried and the symptom. I'll give the smallest hint that unblocks you, not the answer.
