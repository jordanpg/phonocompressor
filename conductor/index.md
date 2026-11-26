# Conductor - phonocompressor

Navigation hub for project context.

## Quick Links

- [Product Definition](./product.md)
- [Product Guidelines](./product-guidelines.md)
- [Tech Stack](./tech-stack.md)
- [Workflow](./workflow.md)
- [Tracks](./tracks.md)

Design, experiment results and decision history: [`docs/PLAN.md`](../docs/PLAN.md).

Tickets: [issues](https://github.com/jordanpg/phonocompressor/issues) (public) on the [project board](https://github.com/users/jordanpg/projects/8) (private). How tracks and tasks map to issues: [`workflow.md`](./workflow.md#issues-and-the-project-board).

## Style Guides

- [General](./code_styleguides/general.md)
- [Rust](./code_styleguides/rust.md)
- [Python](./code_styleguides/python.md) (`tools/` harness)
- [TypeScript](./code_styleguides/typescript.md) (planned web tool)

Each guide starts with a "phonocompressor overrides" section; the repo's tool configs (`rustfmt.toml`, `[tool.ruff]`, the `mise.toml` tasks at the root and in `tools/`) win over the guides.

## Active Tracks

<!-- Auto-populated by /conductor:new-track -->

_None. Run `/conductor:new-track` to start one._

## Archived Tracks

- [x] [Build the WASM binding](./tracks/_archive/wasm-binding_20261125/index.md) (`wasm-binding_20261125`, [#14](https://github.com/jordanpg/phonocompressor/issues/14), part of [#1](https://github.com/jordanpg/phonocompressor/issues/1)): `crates/wasm` binding, CLI parity check and a Worker test page. 9/9 tasks, complete.

## Project Status

- Phases 0–1 done: native CLI works end to end on `eleven_flash_v2_5`.
- Phase 2 partly done (KN G2P shipped; reading model N not started).
- Phase 3: WASM binding built (`wasm-binding_20261125`); the front end and deploy are next.

## Commands

```sh
mise run data                                          # build data/generated/phono.bin
mise run phono compress 'bad dream'                    # use the engine
mise run fmt:check && mise run lint && cargo test --workspace
mise run phono eval                                    # engine regression check
```

## Getting Started

Run `/conductor:new-track` to create your first feature track.
