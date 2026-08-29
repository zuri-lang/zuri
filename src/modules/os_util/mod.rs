//! Heavier, stateful native support for the `os` module
//! (`src/modules/os.rs`); the same split `compress`/`compress_util`
//! and `isolate`/`isolate_util` already use for their own bulkier
//! pieces. `sysinfo` covers per-platform machine/process facts,
//! `process` wraps a spawned child process, and `signal` handles
//! trapping OS signals for graceful shutdown.

pub mod process;
pub mod signal;
pub mod sysinfo;
