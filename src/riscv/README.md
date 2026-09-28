# Rust RISC-V instructions

This module implements the **196 named instructions in `rvgen-go/riscv`**, using
that implementation and `refer/riscv-py` as references. This includes their
RV32/RV64 integer, multiply/divide, atomic, floating-point, compressed, CSR,
instruction-fence, privileged, and Svinval instructions. It does not claim to
implement every instruction in newer revisions of those extensions.

## Store instructions in a basic block

`Instruction` is a `Copy` enum. Each variant contains only the fields it needs,
so there is no separate opcode/operand pairing to keep consistent. Integer and
floating-point registers have different types. No trait object is needed.

```rust
use crate::riscv::{Instruction, Opcode, XReg, Xlen};

fn example() -> Result<(), Box<dyn std::error::Error>> {
    let mut instrs: Vec<Instruction> = vec![
        Instruction::Lui { rd: XReg::T0, imm: 0x12345 },
        Instruction::Addi { rd: XReg::T0, rs1: XReg::T0, imm: -16 },
        Instruction::CAddi { rd: XReg::T0, imm: 1 },
        Instruction::nop(),
    ];

    // Operands stay editable until emission (e.g. when fixing branch offsets).
    if let Instruction::Addi { imm, .. } = &mut instrs[1] {
        *imm = 24;
    }

    let mut rng = rand::rng();
    instrs.push(Opcode::Add.random(&mut rng, Xlen::X64)?);

    let mut bytes = Vec::new();
    for instruction in &instrs {
        instruction.encode_for(Xlen::X64)?.append_bytes(&mut bytes);
    }

    // The existing BasicBlock field Vec<Box<Instruction>> works as well.
    let boxed: Vec<Box<Instruction>> = instrs.into_iter().map(Box::new).collect();
    assert_eq!(boxed[2].byte_len(), 2);
    Ok(())
}
```

Prefer `Vec<Instruction>` for ordinary blocks: it stores the values together and
avoids a heap allocation per instruction. Keep `Box<Instruction>` when separate
ownership or stable addresses are useful. This change leaves the existing
`BasicBlock` and all files outside `src/riscv` untouched.

## Operand conventions

- Use ABI constants (`XReg::A0`, `FReg::FA0`), numbered constants (`XReg::X10`,
  `FReg::F10`), or checked constructors (`XReg::new(10)?`, `Csr::new(0x300)?`).
  Named CSR constants include `Csr::MSTATUS`, `Csr::FFLAGS`, and the reference's
  counter/PMP addresses. `abi_name()` returns the register's ABI spelling.
- Ordinary arithmetic/load/store immediates are signed 12-bit values. Branch
  and jump immediates are **signed byte offsets**, aligned to two bytes.
- `Lui` and `Auipc` take the **upper 20-bit field**, not a byte address; signed
  20-bit values and unsigned 20-bit patterns are both accepted. `CLui` takes a
  nonzero signed 6-bit upper field. `asmutil::load_imm32` handles splitting and
  sign extension for loading a signed 32-bit value on either XLEN.
- Compressed memory immediates are **byte offsets** with the required alignment.
  Registers are architectural numbers (x8 through x15 for compact register
  fields), not three-bit register indices. `CAddi16sp` needs only `imm`, since
  its register is fixed to x2. Compressed shifts use `shamt`, and require a
  nonzero shift. Use `Instruction::cnop()` for the canonical compressed NOP.
- Atomic instructions take `ordering: Ordering::{Relaxed, Release, Acquire,
  AcqRel}`. LR has no `rs2` field. FP instructions use `RoundingMode`, including
  `Dyn`; reserved modes 5 and 6 cannot be constructed. The exact widening
  conversions retain the reference's `rm` operand; use `Rne` for canonical
  assembler output.
- CSR immediates use `uimm: u8` and must fit five bits. `Fence { imm: u16 }`
  takes the full 12-bit field; `FenceOrdering::RwRw.bits()` and the other named
  orderings provide common values. `FenceI` has no operands.

```rust
use crate::riscv::{Csr, FReg, Instruction, Ordering, RoundingMode, XReg};

let instructions = [
    Instruction::LrW {
        rd: XReg::T0, rs1: XReg::A0, ordering: Ordering::Acquire,
    },
    Instruction::FaddS {
        rd: FReg::FA0, rs1: FReg::FA1, rs2: FReg::FA2, rm: RoundingMode::Dyn,
    },
    Instruction::Csrrwi { rd: XReg::ZERO, csr: Csr::FFLAGS, uimm: 0 },
];
```

## Encoding and selection

- `encode()` returns `Result<EncodedInstruction, EncodeError>`. It validates
  operand ranges and compressed register restrictions without choosing an XLEN.
- `encode_for(Xlen::X32)` additionally rejects RV64-only instructions and shifts
  above 31. `encode_for(Xlen::X64)` rejects `CJal`, which is RV32-only. Word
  shifts always fit five bits. Enabled extensions, privilege, memory addresses,
  control-flow targets, and execution state remain the caller's responsibility.
- `EncodedInstruction::{Standard(u32), Compressed(u16)}` carries its width.
  `bits()`, `byte_len()`, `to_le_bytes()`, and `append_bytes()` provide emission.
  Display prints an assembler directive, such as `.4byte 0xfff00093`.
- `instruction.append_bytes(&mut bytes)` performs the same checks as `encode()`
  and leaves the destination unchanged on error. Use `encode_for` followed by
  encoded `append_bytes` when the target XLEN is known.
- `Instruction::standard(word)` and `Instruction::compressed(halfword)` store
  raw bits and bypass validation, including for deliberate illegal encodings.
- `Opcode::ALL`, `class()`, `extension()`, `supports_xlen()`, and
  `InstructionClass::opcodes()` support selection. `"addi".parse::<Opcode>()`
  looks up an exact mnemonic; it does not parse assembly operands.
- `opcode.random(&mut rng, xlen)` generates encodable operands for every named
  opcode, including C instructions. Standard register sampling uses the
  reference's temporary/argument register pools. It does not arrange safe
  execution; loads, CSR accesses, and control transfers can trap or jump away.
- `rvwmo` retains the reference's syntactic dependency roles, accumulating CSR
  flags, and memory-operation groupings. It is metadata, not an execution model;
  `syntactic_dependency()` returns `None` for forms absent from that table.

## Implementation and intentional reference differences

`instruction.rs` contains one declarative catalog. A macro produces the enum,
opcode metadata, random constructors, and encoding dispatch. `encoding.rs`
shares bit packers across instructions; `operand.rs` shares validation and
sampling bounds. `fields.rs`, `registers.rs`, `asmutil.rs`, and `rvwmo.rs` contain
operand types and architectural helpers. Adding an instruction usually means
adding one catalog entry and any genuinely new operand restrictions.

The API reports invalid operands instead of silently masking out-of-range
immediates or dropping misaligned offset bits. It uses canonical encodings for
`FenceI` and the following corrections:

- `C.EBREAK` is `0x9002`; the reference uses the wrong low opcode bits.
- `C.SDSP` uses offset bits 5:3, rather than 4:2 as in the references.

Both corrections match GNU Binutils 2.46 and the
[RISC-V compressed instruction specification](https://docs.riscv.org/reference/isa/v20260120/unpriv/c-st-ext.html).
`Csr::MCONTEXT` is also corrected to `0x7a8`, as specified by
[the RISC-V debug specification](https://docs.riscv.org/reference/debug/Sdtrig.html),
instead of duplicating the reference's `TDATA3` address.
`PrivilegeLevel` contains U, S, and M; the reference's older `Hypervisor = 2`
entry is omitted because that encoding is
[reserved](https://docs.riscv.org/reference/isa/v20260120/priv/priv-intro.html).

## Validation

Run `cargo test --offline`. The module's tests cover:

- 1,568 independently assembled encodings (eight sets for each of 196 opcodes);
- random generation for every supported opcode on both XLENs;
- invalid ranges, alignment, register subsets, CSR fields, and XLEN restrictions;
- operand edits, boxed/value storage, mixed widths, and little-endian emission;
- immediate loading at signed and carry boundaries, including RV64 sign extension.

See [testdata/README.md](testdata/README.md) for fixture provenance and verification.
No new Cargo dependencies or changes outside this directory are required.
