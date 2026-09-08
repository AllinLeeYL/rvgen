`rust_runtime.json` records SHA-256 hashes of complete ELF files produced by
`rvgen::runtime::executable` in the Rust implementation at migration time.

All images use entry address `0x80000100`, in both RV32 and RV64 modes. Bodies are:

- `empty`: zero bytes.
- `compressed`: bytes `01 00` (C.NOP).
- `illegal`: four `ff` bytes.
- `long`: 2,048 repetitions of the little-endian word `0x00000013` (NOP).

Tests compare complete files, including runtime instructions, padding, headers,
sections, mailboxes, string tables, and symbols. No Rust toolchain is needed to
run the tests.
