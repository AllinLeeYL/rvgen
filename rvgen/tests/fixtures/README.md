These fixtures were captured from the Python source in `../../../src/rvgen`
when creating the Rust port. Cargo tests consume them directly; Python is not
a build or test dependency.

- `encodings.tsv`: encoder name, comma-separated arguments, and hexadecimal
  output, separated by tabs. It contains 32 cases for each of the 212 functions
  in the instruction modules and `rvprotoinstrs`, using Python `random.Random(7481)`
  together with directed register and immediate boundaries.
- `empty32.bin` and `empty64.bin`: the Python ELF writer with a single empty
  `.text` section, default flags/alignment/address, and entry `0x80000000`.
- `sections32.bin` and `sections64.bin`: `.text` at `0x80000000` containing bytes
  `13 00 00 00 73 00 10 00`, `.data` at `0x80001000` containing bytes 0 through 16
  with flags 3 and alignment 16, and non-allocated `.comment` containing
  `fixture\0` with alignment 1. Entry is `0x80000000`.
- `assembly.S`: `Generator(GeneratorParams()).gen_assembly(...)` using the
  original Jinja2 template.
- `runtime32.bin` and `runtime64.bin`: fixed Python runtime ELFs with an empty
  workload, executable entry at `0x80000000`, HTIF mailboxes, and a symbol table.
  Unlike the original `empty*.bin` builder fixtures, these execute and stop Spike.

Encoder parity preserves the Python bit layouts, including its existing
compressed-helper quirks. It does not assert architectural validity of all
random operands or establish ISA conformance.
