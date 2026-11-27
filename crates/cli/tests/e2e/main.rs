//! End-to-end tests: run the built `phono` binary and check its exit codes and output.
//!
//! The tests in `no_data` run under plain `cargo test`. The rest need a data file built from
//! this commit, so they are ignored by default: `mise run //crates/cli:e2e` builds it (`mise run
//! data`) and runs them all. `PHONO_DATA` points them at another data file.
//!
//! They check properties that should survive retuning the weights and risks (JSON shape,
//! budgets, ranking order, agreement between commands, error messages), not exact answers. A few
//! quality floors in `compress` are the exception.
//!
//! Phrases and fixtures are public or made up, as in `crates/wasm/parity/cases.json`: never
//! anything from `sample/`.

mod commands;
mod common;
mod compress;
mod harness;
mod no_data;
