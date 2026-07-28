# Litter patch for Iroh 1.0.3

This directory contains the published `iroh 1.0.3` crate source from crates.io.
It is selected by the workspace's `[patch.crates-io]` section.

Codex 0.145.0 depends on `starlark 0.14.2`, which pins `blake3` to exactly
1.8.2. Iroh 1.0.3 normally requires 1.8.3, while Iroh 0.98.1 cannot coexist
with Codex's stable `sha2 0.11` dependency.

The local patch:

- relaxes Iroh's `blake3` requirement from 1.8.3 to 1.8.2; and
- replaces two `Hash::as_slice()` calls with the equivalent
  `Hash::as_bytes()` API available in BLAKE3 1.8.2.

No Iroh protocol or runtime behavior is otherwise changed.
