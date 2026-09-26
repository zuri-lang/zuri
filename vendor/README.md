# Patched crates

Zuri builds against a few crates with changes of its own. Each change is
kept here as a patch against the published crate, and `cargo patch-crates`
builds the patched copies the workspace compiles:

```console
$ cargo patch-crates
patched cranelift-codegen 0.134.4
```

Run it once before the first build, and again whenever a patch changes.
Until it has run, Cargo cannot load the workspace and stops with
`failed to read vendor/crates/<crate>/Cargo.toml`. When a copy is older
than its patch, the build stops and says so.

## Layout

| Path | What it holds |
| --- | --- |
| `patches/<crate>-<version>.patch` | the change, against that exact published version |
| `crates/<crate>` | the patched copy `[patch.crates-io]` points at, not committed |
| `tool` | `cargo patch-crates` itself |

The tool takes each crate from Cargo's registry cache, downloading it
first if need be, so the copy starts from the same verified bytes a
normal dependency would. It keeps its own workspace, because the main
workspace does not load until the copies exist.

## Changing a patch

Edit the copy in `crates/<crate>` directly and build as usual. When the
change is done, write it back out:

```console
$ cargo patch-crates save cranelift-codegen
saved vendor/patches/cranelift-codegen-0.134.4.patch
```

`cargo patch-crates --check` lists copies that are missing or older than
their patch, and exits 1 if there are any.

## Moving to a new version

```console
$ cargo patch-crates upgrade cranelift-codegen 0.135.0
```

This starts the copy over from the new version and applies the current
patch to it. Hunks that no longer fit are left beside their files as
`.rej`. Make each change by hand and delete the `.rej`, then run
`cargo patch-crates save cranelift-codegen`, which writes the patch
under the new version and removes the old one. Finally, move every
`cranelift-*` dependency in `Cargo.toml` to the new version, since
Cranelift's crates are released together.

## cranelift-codegen

Cranelift's code generator with one change to its x64 backend: jump
placement for Intel cores with the jump conditional code erratum
(Skylake through Cascade Lake). With the microcode that fixes the erratum
installed, such a core keeps any 32-byte block of code holding a jump
that crosses or ends on a 32-byte boundary out of its decoded-uop cache,
so a hot loop's speed depends on where its jumps happen to land.

`cranelift_codegen::isa::x64::set_place_jumps(true)` turns the placement
on. Each jump is then placed clear of those boundaries, a conditional
jump together with the instruction before it, which it fuses with, by
padding with nops in front of them. The padding before a fused pair goes
in through `MachBuffer::insert_padding`, which moves the code after the
insertion point along with its label fixups and source locations, and
declines when anything else refers to that code by offset.

The JIT turns it on only when CPUID reports an affected core
(`jump_erratum` in `src/jit/engine.rs`), together with 32-byte function
alignment. Everywhere else the backend emits exactly what upstream does.

The patch changes `src/isa/x64/mod.rs`, `src/isa/x64/inst/emit.rs`,
`src/isa/x64/inst/emit_state.rs` and `src/machinst/buffer.rs`.
