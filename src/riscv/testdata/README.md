# GNU encoding fixtures

`gnu_encodings.tsv` contains 1,568 cases: eight operand sets for every one of the
196 named opcodes in the reference Go table. It was produced with GNU Binutils
2.46 (RISC-V `as`, `ld`, and `objcopy`), independently of the Rust encoders and
of the old migration fixtures. Cases include negative and boundary immediates,
register endpoints, compressed offsets, all valid rounding modes where GNU
syntax permits them, all atomic orderings, and CSR address endpoints.

Columns are tab-separated: mnemonic, XLEN, expected hex word, comma-separated
operands in Rust catalog order, and exact assembler source. The test-only
catalog adapter constructs each typed enum variant from those operand values.
The final column makes each expected word independently reviewable.

`CJal` is assembled for RV32; other cases use RV64. Random generation and
XLEN rejection are tested separately for both targets. Exact conversions
`fcvt.d.s`, `fcvt.d.w`, and `fcvt.d.wu` use RNE in these fixtures because GNU's
canonical syntax omits their rounding-mode operand.

Standard instructions use `.option norvc` to prevent automatic compression;
compressed instructions use `.option rvc`. Both assembler and linker relaxation
are disabled. Linking at `0x100000` resolves PC-relative `.+offset` expressions,
including negative boundary offsets, before extracting `.text`.

To verify the frozen fixture with a RISC-V GNU toolchain on PATH:

```sh
python3 src/riscv/testdata/verify_gnu.py
```

For a different toolchain location or prefix:

```sh
python3 src/riscv/testdata/verify_gnu.py \
  --prefix /path/to/bin/riscv64-unknown-elf-
```

The script assembles the stored source in a temporary directory and compares
every resulting word. It does not modify the fixtures or invoke Rust. Ordinary
`cargo test` uses the checked-in words and needs no assembler or Python.
