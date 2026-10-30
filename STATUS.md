# Progressive Project Practices — Status

## Track: async
Started: 2026-05-20

## Current
- L02 — Tokio runtime + structured spawning — **brief written**, ready to start (worker-pool, no live HTTP). Awaiting user `next` ping.

## Completed
- [x] L01 — Runtime mental model — completed 2026-09-02 — review report: [REVIEWS/L01.md](REVIEWS/L01.md)

## Roadmap
- [x] L01 — Runtime mental model — toy single-threaded executor _(done 2026-09-02)_
- [ ] L02 — Tokio runtime + structured spawning — bounded-concurrency worker-pool (simulated jobs; offline-friendly)
- [ ] L03 — Async I/O + graceful shutdown — TCP echo server
- [ ] L04 — Channels — actor-pattern download manager
- [ ] L05 — Sync primitives — rate-limited API client
- [ ] L06 — Streams & sinks — WebSocket log tailer
- [ ] L07 — Cancellation, timeouts, select — deadline scraper
- [ ] L08 — Custom Future + Pin — `Delay` + `Race` combinator
- [ ] L09 — Async traits (AFIT vs `async-trait`) — pluggable Transport
- [ ] L10 — Observability — `tracing` + `tokio-console`
- [ ] L11 — Axum + Tower deep dive — REST + SSE service
- [ ] L12 — sqlx — persistence layer with transactions
- [ ] L13 — tonic gRPC — unary + streaming RPC
- [ ] L14 — Production patterns — orchestration + graceful shutdown
- [ ] L15 — Capstone — profile, bench, deliver

## Notes
_(persistent observations across lessons go here)_
