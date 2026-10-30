# Progressive Project Practices

Self-paced training workspace for becoming an expert in async Rust (track A) and, later, embedded Rust (track B).

Driven by the `rust-mentor` + `rust-async-curriculum` skills.

## Layout

```
.
├── Cargo.toml          # workspace root (shared lints, deps, profiles)
├── rust-toolchain.toml # pinned stable channel
├── deny.toml           # cargo-deny config
├── STATUS.md           # current cursor + completed lessons
├── REVIEWS/            # one Markdown report per graduated lesson
└── lessons/
    └── lNN-<slug>/     # per-lesson cargo crate, each with its own README (the Lesson Brief)
```

## Workflow

1. Mentor emits a **Lesson Brief** at `lessons/lNN-*/README.md`.
2. User implements the project under that crate.
3. User pings `review` — mentor runs the tool sweep, gives must-fix / nice-to-have feedback, gates graduation.
4. On clean review + explicit `next`, mentor emits the next brief.

## Commands

```bash
# Per-crate iteration
cargo build  -p l01-runtime
cargo run    -p l01-runtime
cargo test   -p l01-runtime
cargo clippy -p l01-runtime --all-targets -- -D warnings

# Workspace-wide
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo doc --workspace --no-deps --open
```

## Current status
See [STATUS.md](STATUS.md).

## Offline preparation (before a flight or any no-Internet session)

`cargo doc` output is identical to docs.rs — the site literally runs `cargo doc` on published crates. So a local build gives you a full offline mirror for every crate in your dependency graph.

### Prewarm before losing Internet

```bash
# 1. Confirm rustup docs are installed (std + all books)
rustup doc --path
# If missing: rustup component add rust-docs

# 2. Full docs for this workspace + every transitive dep
cd ~/Desktop/rust/progressive-project-practices
cargo doc --workspace --all-features
# Output: target/doc/<crate>/index.html

# 3. Cache dep sources so cargo add/build works offline for pinned versions
cargo build --workspace --all-features
cargo fetch                                    # pulls all deps to ~/.cargo/registry/cache/
```

### Docs for crates NOT in the workspace (scratch project trick)

```bash
mkdir -p ~/scratch/docs-mirror && cd ~/scratch/docs-mirror
cargo init --lib
cargo add tokio --features full     # or axum, sqlx, tonic, hyper, reqwest, serde, ...
cargo doc --all-features --no-deps  # docs for THIS project's direct deps
# Browse: ~/scratch/docs-mirror/target/doc/tokio/index.html
```

### Reading docs offline

- **std + books**: `rustup doc --std` / `rustup doc --book` / `rustup doc --reference` — opens local rendered HTML.
- **Workspace deps** (tokio, tokio-util, pin-project-lite, clap, tracing, anyhow, futures, thiserror, tracing-subscriber, console-subscriber): `xdg-open target/doc/tokio/index.html` (or the crate you want).
- **Search bar** in the top-right of any generated crate page works offline — local JS.
- Doc navigation between crates works via file:// links, so cross-crate hyperlinks resolve.

### What DOES NOT work offline

- `docs.rs` the website
- `cargo search <name>` (hits crates.io index)
- `cargo add <newcrate>` for versions not already in `~/.cargo/registry/cache/`
- `cargo update` (needs index refresh)

Bake anything you might want into the local cache before boarding.

### Suggested browser bookmarks for the plane

```
file:///home/jcmaxwell/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/std/index.html
file:///home/jcmaxwell/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/share/doc/rust/html/book/index.html
file:///home/jcmaxwell/Desktop/rust/progressive-project-practices/target/doc/l01_runtime/index.html
file:///home/jcmaxwell/Desktop/rust/progressive-project-practices/target/doc/l02_spawning/index.html
file:///home/jcmaxwell/Desktop/rust/progressive-project-practices/target/doc/tokio/index.html
file:///home/jcmaxwell/Desktop/rust/progressive-project-practices/target/doc/pin_project_lite/index.html
file:///home/jcmaxwell/Desktop/rust/progressive-project-practices/target/doc/pin_project/index.html
```

Adjust the toolchain triple (`x86_64-unknown-linux-gnu`) to match `rustup doc --path` on your machine.
