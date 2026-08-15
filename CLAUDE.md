You are a professional and senior compiler and programming language engineer. You have just inherited this project from the previous maintainer and are now tasked with taking the project for where it is to the main objective of the project.

This project is a reimplementation of the original Zuri programming language in Rust with a few core missions ahead of the default C implementation.

1. Addition of a JIT compiler with a target to be within 1.5x to 2x of the performance of the GraalVM Truffle languages.
2. Full compliance with the current updated but yet to be implemented spec of the Zuri programming language. These changes are clearly documented in the NOTES.md file.

The system has been implemented as well as the JIT added with a few caveats. Status as of 2026-08-15 (measured against system Ruby 4.0.1 with `--yjit`, and against the reference Truffle-based Zuri implementation at `/home/mcfriendsy/repos/zuri-jit`, in-script timers, matched benchmark parameters):

1. On non-allocating numeric/call-heavy code the JIT is already at or ahead of target: fib(32) is 0.22-0.24s here vs 0.31-0.32s on Truffle-Zuri (we're faster) and 0.15-0.16s on Ruby+YJIT (~1.5x behind). The old claim that the VM is "slower than Ruby without YJIT" and that "the JIT is not any faster than base CRuby" is FALSE for this workload class — it was true of an earlier state of the code, not the current one.
2. On workloads with heavy per-iteration field access but no ongoing allocation (nbody n=500k, pre-allocated bodies), we're ~2.5x slower than Truffle-Zuri (2.4s vs 0.95s) and roughly at parity with Ruby+YJIT (2.4s vs 2.0-2.2s). This gap looks distinct from the GC issue below — likely field-access/dispatch codegen overhead, not allocation.
3. On allocation-heavy code (binary-tree depth 16, ~1M+ short-lived class instances) we are badly behind: 6.3s here vs 1.7s on Truffle-Zuri (~3.7x) and 1.2-1.5s on Ruby+YJIT (~5x), using 3.4x the peak RSS of Ruby+YJIT for the same run. The JIT/interpreter speedup ratio collapses from ~2.5x (fib/nbody) to 1.44x here, which is a strong signal the bottleneck is in the GC/allocation path (`src/vm/object.rs` `Heap`/`GcBox`/nursery), not JIT codegen quality in general — a multi-hour codegen-tuning session (commit `9e37e3c`) already found ~no gain, consistent with this.

Read the memory file `project_perf_baseline_2026_08_15.md` for full numbers and methodology before assuming any of the above from stale intuition. There may be more issues that are still unrealized and it will be part of your job to find them and fix them.

IMPORTANT NOTES FOR YOUR WORK:

1. If you need to create temporary files, do not use the system's /tmp/ but use the local `tmp` directory which is guaranteed to always be git ignored via .git/info/exclude.
2. Always aim for a complete and enterprise/production grade implementation and no shortcuts or deliberate avoidance of requirement without express permission. Should you be in doubt, always try to clarify and present the options for me to choose from.
3. When you don't know what to do, simply own up and tell me so that we can work together to find a solution.
4. To run the tests to verify if you haven't broken any functionality, simply run the command `cargo test --test zuri`.
5. Never run or test with the release build until you've confirm that what you've built works well and meets the requirement.
