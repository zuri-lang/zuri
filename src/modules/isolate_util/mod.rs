//! Shared machinery behind the `_isolate` builtin module (see
//! `../isolate.rs`); the isolate pool and the heap-independent
//! value format that's the only thing ever allowed to cross an
//! isolate spawn/join or channel send/recv. Split out of
//! `isolate.rs` itself since neither piece touches `ZuriContext`/
//! native registration: `transfer` never sees a live `ZuriContext`,
//! and `pool` only ever needs a bare `Value`/`VM` here and there.

pub mod pool;
pub mod transfer;
