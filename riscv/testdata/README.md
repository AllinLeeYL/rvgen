`rust_encodings.json` was captured from the repository's Rust low-level encoders
using eight operand sets per opcode (196 opcodes, 1,568 cases). Sets vary signed
immediates, register numbers, shift amounts, rounding modes, CSR addresses, and
atomic ordering. `Panics` records either a low-level assertion or a compressed
result wider than 16 bits. `Instruction.Kind` follows the Go opcode table order.

These are migration fixtures, not an independent ISA specification. The separate
`TestGNUReferenceEncodings` test uses the reference words in the Rust instruction
tests, which were checked against GNU RISC-V tools. Tests do not regenerate these
fixtures or invoke Rust. Intentional encoding changes should be checked against
an ISA reference before fixtures are updated.
