# Working on DiskMate

<!-- LOVABLE:BEGIN -->
> [!IMPORTANT]
> This project is connected to [Lovable](https://lovable.dev). Avoid rewriting
> published git history — force pushing, or rebasing/amending/squashing commits
> that are already pushed — as it rewrites history on Lovable's side and the
> user will likely lose their project history.
>
> Commits you push to the connected branch sync back to Lovable and show up in
> the editor, so keep the branch in a working state.
<!-- LOVABLE:END -->

DiskMate is a native macOS application built with Tauri 2, Rust and React.
Lovable created the original placeholder repository and is no longer part of
building or running it. GitHub is the source of truth.

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing anything structural.

## Rules

**Milestone 01 is read-only.** No code may delete, move or modify a file, change
a launch item, or signal a process. When cleanup is built, every destructive
operation goes through one service — see [docs/security.md](docs/security.md).

**Product logic lives in `crates/diskmate-core`.** Not in React components, and
not in Tauri command handlers. A handler deserialises, calls one service method,
and serialises.

**The UI never receives a filesystem tree.** It asks the index bounded
questions. Any query that could return an unbounded result is a bug.

**Nothing is invented.** No fabricated counts, no estimated measurements, no
fake progress, no alarm styling. If something is not implemented, the page says
so and shows nothing. If something cannot be measured, it reports as unmeasured.

**Do not add dependencies casually.** The scaffold this repository started from
carried around fifty unused ones. Prefer writing fifty lines to taking a library
for a trivial problem.

**Do not introduce** Supabase, Lovable Cloud, Stripe, authentication, AI
features, external analytics, or any requirement for network connectivity.

**Keep macOS-specific code behind `platform::SystemProbe`.** The engine builds
and its tests run on Linux, which is how it stays honest about that boundary.

## Before pushing

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck && npm run lint && npm run test
```

Changes to the scanner should come with a benchmark run:

```sh
./scripts/bench.sh
```

Do not quote performance numbers that have not been measured on the machine
being described.
