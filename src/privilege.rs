//! Switching the hart among M, S and U-mode, after Cascade.
//!
//! Every switch is an edge between two consecutive blocks, so each block still
//! runs once, in order, and Spike's probes keep their meaning:
//!
//! - *Descending* ends a block with `mret` or `sret`, its exception PC pointed
//!   at the next block and MPP/SPP at the mode the next block runs in.
//! - A *trap* ends a block with an instruction that raises an exception
//!   (`ecall`, `ebreak`, an illegal instruction, a misaligned or PMP-denied
//!   access, ...). The handling mode's trap vector points at the next block, a
//!   *landing* block, which restores the vector and reads the cause and
//!   exception PC into registers the next guard checks against Spike.
//!
//! A mode can only arm its own trap vector, so leaving M arms `mtvec` for the
//! landing of the trap that will eventually return to M, and entering U arms
//! `stvec` too (when S exists) for traps that `medeleg` hands to S. While the
//! hart is below M, `mtvec` is always armed. A disarmed vector points at a
//! *sink* handler (`_trap_handler`, `_strap_handler`) that ends the test, so an
//! unplanned trap still fails loudly; an unplanned trap through an armed vector
//! lands early, where the exception PC read no longer matches Spike's.
//!
//! The landing addresses are absolute, so the trap vectors and exception PCs
//! are entangled value sites like `jalr` targets. Traps are routed by the
//! `medeleg` the generator wrote, restricted to `--medeleg-mask`: the bits both
//! the device and Spike delegate. `ecall` from S is never delegated, so S can
//! always return to M; the program returns to M before `_check`.
//!
//! The device's PMP has one allow-all entry last, so S and U can reach all
//! memory, and lower entries cover 4 KiB regions of a private window with
//! random permissions, which planned S/U accesses violate (access faults).
//! Other options switch the FPU off (FP instructions trap), set
//! `mstatus.TSR` (`sret` in S traps) and `mstatus.TVM` (`sfence.vma` and
//! `satp` in S trap), or change `medeleg`.
use anyhow::{Result, ensure};
use rand::{Rng, RngExt};

use crate::basicblock::BasicBlock;
use crate::entangle::{Dest, Kind, Site, SiteBuilder};
use crate::membase::MemBases;
use crate::riscv::asmutil::{load_imm, load_imm32};
use crate::riscv::{Csr, Extension, Instruction, InstructionClass, Opcode, PrivilegeLevel, XReg};
use crate::target::Target;

use PrivilegeLevel::{Machine as M, Supervisor as S, User as U};

/// Landing keys of the sink handlers: `_trap_handler` (M) and
/// `_strap_handler` (S).
pub const SINK_M: usize = usize::MAX;
pub const SINK_S: usize = usize::MAX - 1;

// mstatus fields.
const SIE: u64 = 1 << 1;
const MIE: u64 = 1 << 3;
const SPIE: u64 = 1 << 5;
const MPIE: u64 = 1 << 7;
const SPP: u64 = 1 << 8;
const MPP: u64 = 3 << 11;
pub const FS: u64 = 3 << 13;
const MPRV: u64 = 1 << 17;
const SUM: u64 = 1 << 18;
const MXR: u64 = 1 << 19;
const TVM: u64 = 1 << 20;
const TW: u64 = 1 << 21;
const TSR: u64 = 1 << 22;

/// mstatus fields `_init` clears: no interrupt enabled, no memory or trap
/// virtualization, previous modes U. Only `_init` and the switches here write
/// mstatus.
pub const MSTATUS_CLEARED: u64 = SIE | MIE | SPIE | MPIE | SPP | MPP | MPRV | SUM | MXR | TVM | TW | TSR;

/// Alignment of a landing reached through a vectored trap vector: BASE's low
/// bits may be ignored in vectored mode (Rocket ignores 8), though not by
/// Spike, and exceptions go to BASE either way.
pub const VECTORED_LANDING_ALIGN: u64 = 256;

/// Size and alignment of one PMP window region (NAPOT), coarser than any
/// PMP granularity in practice.
pub const PMP_REGION: u64 = 4096;
/// Most window regions; each takes a PMP entry.
pub const MAX_PMP_WINDOW_REGIONS: usize = 4;

/// PMP permission bits.
pub const PMP_R: u8 = 1;
pub const PMP_W: u8 = 2;
pub const PMP_X: u8 = 4;

/// Exception causes the generator raises on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    InstrMisaligned = 0,
    InstrAccess = 1,
    Illegal = 2,
    Breakpoint = 3,
    LoadMisaligned = 4,
    LoadAccess = 5,
    StoreMisaligned = 6,
    StoreAccess = 7,
    EcallU = 8,
    EcallS = 9,
    EcallM = 11,
}

impl Cause {
    const ALL: [Cause; 11] = [
        Cause::InstrMisaligned,
        Cause::InstrAccess,
        Cause::Illegal,
        Cause::Breakpoint,
        Cause::LoadMisaligned,
        Cause::LoadAccess,
        Cause::StoreMisaligned,
        Cause::StoreAccess,
        Cause::EcallU,
        Cause::EcallS,
        Cause::EcallM,
    ];
}

/// `medeleg` bits the generator never sets: `ecall` from S, so S can always
/// trap to M, and `ecall` from M, which is read-only zero.
const NEVER_DELEGATED: u64 = 1 << Cause::EcallS as u64 | 1 << Cause::EcallM as u64;

/// The privileged state the generated code has set up so far.
#[derive(Debug, Clone)]
pub struct PrivState {
    pub mode: PrivilegeLevel,
    /// Landing key `mtvec` is armed with; `None` when it points at the sink.
    pub mtvec: Option<usize>,
    pub stvec: Option<usize>,
    pub medeleg: u64,
    /// Whether `mstatus.FS` enables the FPU.
    pub fs_on: bool,
    pub tsr: bool,
    pub tvm: bool,
}

impl Default for PrivState {
    fn default() -> Self {
        Self { mode: M, mtvec: None, stvec: None, medeleg: 0, fs_on: true, tsr: false, tvm: false }
    }
}

impl PrivState {
    /// Whether the workload may use `opcode` in the current state without
    /// trapping.
    pub fn allows(&self, opcode: Opcode) -> bool {
        use InstructionClass::*;
        if matches!(opcode.class(), FloatMemory | Float | Float64 | DoubleMemory | Double | Double64) {
            return self.fs_on;
        }
        match opcode {
            Opcode::SfenceVma | Opcode::SinvalVma | Opcode::SfenceWInval | Opcode::SfenceInvalIr => {
                self.mode == M || self.mode == S && !self.tvm
            }
            Opcode::Wfi => self.mode == M,
            _ => true,
        }
    }

    /// Whether the current mode may access `csr`: its address encodes the
    /// lowest privilege allowed, and the FP CSRs need the FPU on.
    pub fn allows_csr(&self, csr: Csr) -> bool {
        let level = (csr.index() >> 8) & 3;
        let fp = matches!(csr.index(), 0x001..=0x003);
        level <= self.mode as u16 && (!fp || self.fs_on)
    }
}

/// Private memory whose 4 KiB regions the low PMP entries cover with
/// `perms` (R, W, X bits), for planned access faults in S and U.
#[derive(Debug, Clone)]
pub struct PmpWindow {
    pub start: u64,
    pub perms: Vec<u8>,
}

impl PmpWindow {
    /// The start of a random region lacking permission `bit`, if any.
    fn region_without(&self, bit: u8, rng: &mut (impl Rng + ?Sized)) -> Option<u64> {
        let regions: Vec<_> = (0..self.perms.len()).filter(|&i| self.perms[i] & bit == 0).collect();
        (!regions.is_empty())
            .then(|| self.start + regions[rng.random_range(0..regions.len())] as u64 * PMP_REGION)
    }
}

/// The landing a block ended with: the next block must start with
/// [`Switcher::land`].
#[derive(Debug, Clone, Copy)]
pub struct Landing {
    pub key: usize,
    pub handler: PrivilegeLevel,
}

/// What generator code may use around a block: the sites, if entangling, and
/// otherwise the registers it must not write.
pub struct Ctx<'a, 'b> {
    pub target: &'a Target,
    pub sites: Option<&'a mut SiteBuilder<'b>>,
    pub reserved: &'a [XReg],
    pub mem_bases: &'a MemBases,
}

impl Ctx<'_, '_> {
    fn scratch(&mut self, rng: &mut (impl Rng + ?Sized)) -> XReg {
        match self.sites.as_deref_mut() {
            Some(sites) => sites.scratch(rng),
            None => loop {
                let reg = XReg::new(rng.random_range(1..32)).expect("valid register");
                if !self.reserved.contains(&reg) {
                    return reg;
                }
            },
        }
    }

    /// `reg = dest`: an entangled value site, or `auipc; addi` without sites.
    fn address(&mut self, bb: &mut BasicBlock, dest: Dest, rng: &mut (impl Rng + ?Sized)) -> XReg {
        if let Some(sites) = self.sites.as_deref_mut() {
            let (reg, site) = sites.address(&mut bb.instrs, dest, rng);
            bb.sites.push(site);
            return reg;
        }
        let rd = self.scratch(rng);
        let at = bb.instrs.len();
        bb.instrs.extend([Instruction::nop(), Instruction::nop()]);
        bb.sites.push(Site { at, kind: Kind::Link { rd, offset: 0, dest }, fail_at: None });
        rd
    }

    /// `csr = dest`, in vectored mode (MODE = 1) if `vectored`.
    fn point(&mut self, bb: &mut BasicBlock, csr: Csr, dest: Dest, vectored: bool, rng: &mut (impl Rng + ?Sized)) {
        let reg = self.address(bb, dest, rng);
        if vectored {
            bb.instrs.push(Instruction::Ori { rd: reg, rs1: reg, imm: 1 });
        }
        bb.instrs.push(Instruction::Csrrw { rd: XReg::ZERO, rs1: reg, csr });
    }

    /// Clear then set bits of `csr` (mstatus or sstatus).
    fn status(&mut self, bb: &mut BasicBlock, csr: Csr, clear: u64, set: u64, rng: &mut (impl Rng + ?Sized)) {
        for (bits, set) in [(clear, false), (set, true)] {
            if bits != 0 {
                let reg = self.scratch(rng);
                bb.instrs.extend(load_imm(reg, bits as i64, self.target.xlen));
                bb.instrs.push(if set {
                    Instruction::Csrrs { rd: XReg::ZERO, rs1: reg, csr }
                } else {
                    Instruction::Csrrc { rd: XReg::ZERO, rs1: reg, csr }
                });
            }
        }
    }

    /// Read `csr` into a register: checked by the next guard, or overwritten
    /// right away if its value is implementation-dependent.
    fn read(&mut self, bb: &mut BasicBlock, csr: Csr, checked: bool, rng: &mut (impl Rng + ?Sized)) {
        let rd = self.scratch(rng);
        let instr = Instruction::Csrrs { rd, rs1: XReg::ZERO, csr };
        bb.instrs.push(instr);
        if checked {
            if let Some(sites) = self.sites.as_deref_mut() {
                sites.observe(&instr);
            }
        } else {
            bb.instrs.extend(load_imm32(rd, rng.random::<i32>(), self.target.xlen));
        }
    }

    /// End the block: the instructions leaving it are in place, and falling
    /// through them reaches `_fail`.
    fn leave(&mut self, bb: &mut BasicBlock) {
        let at = bb.instrs.len() - 1;
        bb.instrs.push(Instruction::nop());
        bb.sites.push(Site { at, kind: Kind::Leave, fail_at: Some(at + 1) });
    }
}

/// How a block ends.
#[derive(Debug, Clone, Copy)]
enum Switch {
    /// `mret`/`sret` (`sret` if `via_sret`) into `to`.
    Descend { to: PrivilegeLevel, via_sret: bool },
    /// Switch the FPU off or back on.
    ToggleFs,
    ToggleTsr,
    ToggleTvm,
    /// Write a new `medeleg`.
    Delegate,
}

/// Plans and emits the privilege switches of one program.
pub struct Switcher {
    pub state: PrivState,
    /// Probability that a block ends with a switch, and that one changes the
    /// configuration before it ends.
    switch_proba: f64,
    config_proba: f64,
    /// Relative likelihood of each cause, by [`Cause`] value.
    cause_weights: [f64; 12],
    /// Share of switches that descend, when one can.
    descend_share: f64,
    /// Factor on `switch_proba` below M, so the hart stays longer there.
    lower_stay: f64,
    /// Probability that an armed vector is in vectored mode.
    vectored_proba: f64,
    /// Keys armed in vectored mode, whose landings need more alignment.
    vectored: std::collections::HashSet<usize>,
    lower_modes: bool,
    window: Option<PmpWindow>,
    next_key: usize,
}

impl Switcher {
    /// A per-program plan for `target`. S and U are only entered when the
    /// target has them; `window` holds the PMP-guarded regions, if any.
    pub fn new(target: &Target, window: Option<PmpWindow>, rng: &mut (impl Rng + ?Sized)) -> Self {
        let mut cause_weights = [0.0; 12];
        for cause in Cause::ALL {
            cause_weights[cause as usize] = rng.random::<f64>() + 0.05;
        }
        let mut state = PrivState::default();
        if target.has_privilege(S) {
            state.medeleg = random_medeleg(target, rng);
        }
        Self {
            state,
            switch_proba: rng.random_range(0.0..0.3),
            config_proba: rng.random_range(0.0..0.1),
            cause_weights,
            descend_share: rng.random_range(0.2..0.8),
            lower_stay: rng.random_range(0.25..1.0),
            vectored_proba: rng.random_range(0.0..0.5),
            vectored: Default::default(),
            lower_modes: target.has_privilege(U),
            window,
            next_key: 0,
        }
    }

    pub fn window(&self) -> Option<&PmpWindow> {
        self.window.as_ref()
    }

    /// A fresh landing key, and whether its vector is in vectored mode.
    fn key(&mut self, rng: &mut (impl Rng + ?Sized)) -> (usize, bool) {
        self.next_key += 1;
        let key = self.next_key - 1;
        let vectored = rng.random_bool(self.vectored_proba);
        if vectored {
            self.vectored.insert(key);
        }
        (key, vectored)
    }

    /// Possibly end `bb` with a switch, after possibly changing the
    /// configuration. `last` asks for the hart back in M with its FPU on,
    /// which the self-check needs: the block ends with a trap to M if needed,
    /// and a block in M is left for the caller's plain jump. Returns whether
    /// the block ended, and the landing it ended with, if any.
    pub fn end_block(
        &mut self,
        bb: &mut BasicBlock,
        ctx: &mut Ctx,
        last: bool,
        rng: &mut (impl Rng + ?Sized),
    ) -> Result<(bool, Option<Landing>)> {
        if last {
            if self.state.mode != M {
                let causes = self.causes(ctx, true, rng);
                ensure!(!causes.is_empty(), "no trap returns {:?}-mode code to M", self.state.mode);
                let cause = self.pick_cause(&causes, rng);
                return Ok((true, Some(self.trap(bb, ctx, cause, rng)?)));
            }
            if !self.state.fs_on {
                self.emit(bb, ctx, Switch::ToggleFs, rng)?;
            }
            return Ok((false, None));
        }
        if rng.random_bool(self.config_proba) {
            let configs = self.configs(ctx.target);
            if !configs.is_empty() {
                let config = configs[rng.random_range(0..configs.len())];
                self.emit(bb, ctx, config, rng)?;
            }
        }
        let proba = if self.state.mode == M { self.switch_proba } else { self.switch_proba * self.lower_stay };
        if !rng.random_bool(proba) {
            return Ok((false, None));
        }
        let descents = self.descents(ctx.target);
        let causes = self.causes(ctx, false, rng);
        if descents.is_empty() && causes.is_empty() {
            return Ok((false, None));
        }
        if causes.is_empty() || !descents.is_empty() && rng.random_bool(self.descend_share) {
            let descent = descents[rng.random_range(0..descents.len())];
            Ok((true, self.emit(bb, ctx, descent, rng)?))
        } else {
            let cause = self.pick_cause(&causes, rng);
            Ok((true, Some(self.trap(bb, ctx, cause, rng)?)))
        }
    }

    fn pick_cause(&self, causes: &[Cause], rng: &mut (impl Rng + ?Sized)) -> Cause {
        let total: f64 = causes.iter().map(|c| self.cause_weights[*c as usize]).sum();
        let mut pick = rng.random_range(0.0..total);
        for &cause in causes {
            pick -= self.cause_weights[cause as usize];
            if pick < 0.0 {
                return cause;
            }
        }
        *causes.last().expect("causes is not empty")
    }

    /// Configuration changes possible in the current mode.
    fn configs(&self, target: &Target) -> Vec<Switch> {
        let mut configs = Vec::new();
        let fpu = target.has(Extension::F);
        match self.state.mode {
            M => {
                if fpu {
                    configs.push(Switch::ToggleFs);
                }
                if target.has_privilege(S) {
                    configs.extend([Switch::ToggleTsr, Switch::ToggleTvm]);
                    if target.medeleg_mask & !NEVER_DELEGATED != 0 {
                        configs.push(Switch::Delegate);
                    }
                }
            }
            S if fpu => configs.push(Switch::ToggleFs),
            _ => {}
        }
        configs
    }

    /// Descents possible from the current mode.
    fn descents(&self, target: &Target) -> Vec<Switch> {
        let has_s = target.has_privilege(S);
        let mut descents = Vec::new();
        let mut add = |to, via_sret| descents.push(Switch::Descend { to, via_sret });
        match self.state.mode {
            M => {
                add(M, false);
                if self.lower_modes {
                    add(U, false);
                    if has_s {
                        add(S, false);
                        add(S, true);
                        add(U, true);
                    }
                }
            }
            S if !self.state.tsr => {
                add(S, true);
                add(U, true);
            }
            _ => {}
        }
        descents
    }

    /// The mode handling `cause` raised in the current mode.
    fn handler(&self, cause: Cause, target: &Target) -> PrivilegeLevel {
        if self.state.mode != M && target.has_privilege(S) && self.state.medeleg >> cause as u64 & 1 == 1 {
            S
        } else {
            M
        }
    }

    /// Causes the current mode can raise and take: its handler is its own
    /// mode, which arms its vector right before, or a higher one whose vector
    /// is armed. With `to_m`, only causes handled in M (all of them if none
    /// is, as the hart then gets to M through S).
    fn causes(&self, ctx: &Ctx, to_m: bool, rng: &mut (impl Rng + ?Sized)) -> Vec<Cause> {
        let target = ctx.target;
        let mode = self.state.mode;
        let mut causes: Vec<Cause> = Cause::ALL
            .into_iter()
            .filter(|cause| match cause {
                Cause::EcallU => mode == U,
                Cause::EcallS => mode == S,
                Cause::EcallM => mode == M,
                Cause::Breakpoint | Cause::Illegal => true,
                Cause::InstrMisaligned => !target.has(Extension::C),
                Cause::LoadMisaligned | Cause::StoreMisaligned => {
                    target.misaligned_traps && ctx.mem_bases.misaligned(4, rng).is_some()
                }
                Cause::InstrAccess => mode != M && self.window_lacks(PMP_X),
                Cause::LoadAccess => mode != M && self.window_lacks(PMP_R),
                Cause::StoreAccess => mode != M && self.window_lacks(PMP_W),
            })
            .filter(|cause| {
                let handler = self.handler(*cause, target);
                handler == mode
                    || match handler {
                        M => self.state.mtvec.is_some(),
                        _ => self.state.stvec.is_some(),
                    }
            })
            .collect();
        if to_m && causes.iter().any(|cause| self.handler(*cause, target) == M) {
            causes.retain(|cause| self.handler(*cause, target) == M);
        }
        causes
    }

    fn window_lacks(&self, bit: u8) -> bool {
        self.window.as_ref().is_some_and(|w| w.perms.iter().any(|p| p & bit == 0))
    }

    /// End `bb` with `cause`; the next block lands in its handler.
    fn trap(&mut self, bb: &mut BasicBlock, ctx: &mut Ctx, cause: Cause, rng: &mut (impl Rng + ?Sized)) -> Result<Landing> {
        let mode = self.state.mode;
        let handler = self.handler(cause, ctx.target);
        let key = if handler == mode {
            // Arm this mode's own vector at the next block.
            let (key, vectored) = self.key(rng);
            let (csr, fallback) = if mode == M { (Csr::MTVEC, SINK_M) } else { (Csr::STVEC, SINK_S) };
            ctx.point(bb, csr, Dest::Landing { key, fallback }, vectored, rng);
            key
        } else if handler == M {
            self.state.mtvec.expect("mtvec is armed")
        } else {
            self.state.stvec.expect("stvec is armed")
        };
        self.raise(bb, ctx, cause, rng)?;
        ctx.leave(bb);
        self.state.mode = handler;
        Ok(Landing { key, handler })
    }

    /// The instructions raising `cause` in the current mode.
    fn raise(&self, bb: &mut BasicBlock, ctx: &mut Ctx, cause: Cause, rng: &mut (impl Rng + ?Sized)) -> Result<()> {
        use Instruction::*;
        let target = ctx.target;
        let xlen = target.xlen;
        let mode = self.state.mode;
        match cause {
            Cause::EcallU | Cause::EcallS | Cause::EcallM => bb.instrs.push(Ecall),
            Cause::Breakpoint => bb.instrs.push(if target.has(Extension::C) && rng.random_bool(0.5) {
                CEbreak
            } else {
                Ebreak
            }),
            // Without C, a jump to a halfword boundary: rd is not written.
            Cause::InstrMisaligned => bb.instrs.push(Jal { rd: XReg::ZERO, imm: 2 }),
            Cause::Illegal => {
                let rd = ctx.scratch(rng);
                let mut options = vec![
                    // An all-zero word, and a write to the read-only `cycle` (`unimp`).
                    Raw32 { bits: 0 },
                    Csrrw { rd: XReg::ZERO, rs1: XReg::ZERO, csr: Csr::CYCLE },
                ];
                if mode != M {
                    options.push(Mret);
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::MSCRATCH });
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::MSTATUS });
                }
                if mode == U && target.has_privilege(S) {
                    options.push(Sret);
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::SSCRATCH });
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::SEPC });
                }
                if mode == S && self.state.tsr {
                    options.push(Sret);
                }
                if mode == S && self.state.tvm {
                    options.push(SfenceVma { rs1: XReg::ZERO, rs2: XReg::ZERO });
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::SATP });
                }
                if !self.state.fs_on && target.has(Extension::F) {
                    options.push(Csrrs { rd, rs1: XReg::ZERO, csr: Csr::FCSR });
                    options.push(FaddS {
                        rd: crate::riscv::FReg::new(rng.random_range(0..32))?,
                        rs1: crate::riscv::FReg::new(rng.random_range(0..32))?,
                        rs2: crate::riscv::FReg::new(rng.random_range(0..32))?,
                        rm: crate::riscv::RoundingMode::Rne,
                    });
                }
                bb.instrs.push(options[rng.random_range(0..options.len())]);
            }
            Cause::LoadMisaligned | Cause::StoreMisaligned => {
                let load = cause == Cause::LoadMisaligned;
                let mut sizes = vec![2, 4];
                if xlen == crate::riscv::Xlen::X64 {
                    sizes.push(8);
                }
                let size = sizes[rng.random_range(0..sizes.len())];
                let (base, imm) = ctx
                    .mem_bases
                    .misaligned(size, rng)
                    .ok_or_else(|| anyhow::anyhow!("no room for a misaligned access"))?;
                let reg = ctx.scratch(rng);
                bb.instrs.push(match (load, size) {
                    (true, 2) => Lh { rd: reg, rs1: base, imm },
                    (true, 4) => Lw { rd: reg, rs1: base, imm },
                    (true, _) => Ld { rd: reg, rs1: base, imm },
                    (false, 2) => Sh { rs1: base, rs2: reg, imm },
                    (false, 4) => Sw { rs1: base, rs2: reg, imm },
                    (false, _) => Sd { rs1: base, rs2: reg, imm },
                });
            }
            Cause::InstrAccess | Cause::LoadAccess | Cause::StoreAccess => {
                let bit = match cause {
                    Cause::InstrAccess => PMP_X,
                    Cause::LoadAccess => PMP_R,
                    _ => PMP_W,
                };
                let window = self.window.as_ref().expect("a PMP window");
                let region = window.region_without(bit, rng).expect("a region lacking the permission");
                let addr = region + rng.random_range(0..PMP_REGION / 8) * 8;
                let reg = ctx.scratch(rng);
                bb.instrs.extend(load_imm(reg, addr as i64, xlen));
                let other = ctx.scratch(rng);
                bb.instrs.push(match cause {
                    Cause::InstrAccess => Jalr { rd: XReg::ZERO, rs1: reg, imm: 0 },
                    Cause::LoadAccess => Lw { rd: other, rs1: reg, imm: 0 },
                    _ => Sw { rs1: reg, rs2: other, imm: 0 },
                });
            }
        }
        Ok(())
    }

    /// Emit a descent or a configuration change; a descent returns no
    /// landing, as the next block simply runs in the new mode.
    fn emit(&mut self, bb: &mut BasicBlock, ctx: &mut Ctx, switch: Switch, rng: &mut (impl Rng + ?Sized)) -> Result<Option<Landing>> {
        let status = if self.state.mode == M { Csr::MSTATUS } else { Csr::SSTATUS };
        match switch {
            Switch::ToggleFs => {
                let (clear, set) = if self.state.fs_on { (FS, 0) } else { (0, FS) };
                ctx.status(bb, status, clear, set, rng);
                self.state.fs_on = !self.state.fs_on;
            }
            Switch::ToggleTsr => {
                let (clear, set) = if self.state.tsr { (TSR, 0) } else { (0, TSR) };
                ctx.status(bb, Csr::MSTATUS, clear, set, rng);
                self.state.tsr = !self.state.tsr;
            }
            Switch::ToggleTvm => {
                let (clear, set) = if self.state.tvm { (TVM, 0) } else { (0, TVM) };
                ctx.status(bb, Csr::MSTATUS, clear, set, rng);
                self.state.tvm = !self.state.tvm;
            }
            Switch::Delegate => {
                self.state.medeleg = random_medeleg(ctx.target, rng);
                let reg = ctx.scratch(rng);
                bb.instrs.extend(load_imm(reg, self.state.medeleg as i64, ctx.target.xlen));
                bb.instrs.push(Instruction::Csrrw { rd: XReg::ZERO, rs1: reg, csr: Csr::MEDELEG });
            }
            Switch::Descend { to, via_sret } => {
                let from = self.state.mode;
                // The vectors traps from `to` need: mtvec when leaving M, stvec
                // too when entering U (S may handle its traps).
                if from == M && to != M {
                    let (key, vectored) = self.key(rng);
                    ctx.point(bb, Csr::MTVEC, Dest::Landing { key, fallback: SINK_M }, vectored, rng);
                    self.state.mtvec = Some(key);
                }
                if to == U && ctx.target.has_privilege(S) {
                    let (key, vectored) = self.key(rng);
                    ctx.point(bb, Csr::STVEC, Dest::Landing { key, fallback: SINK_S }, vectored, rng);
                    self.state.stvec = Some(key);
                }
                if via_sret {
                    let spp = if to == S { SPP } else { 0 };
                    ctx.status(bb, status, SPP & !spp, spp, rng);
                    ctx.point(bb, Csr::SEPC, Dest::Next, false, rng);
                    bb.instrs.push(Instruction::Sret);
                } else {
                    let mpp = (to as u64) << 11;
                    ctx.status(bb, Csr::MSTATUS, MPP & !mpp, mpp, rng);
                    ctx.point(bb, Csr::MEPC, Dest::Next, false, rng);
                    bb.instrs.push(Instruction::Mret);
                }
                ctx.leave(bb);
                self.state.mode = to;
            }
        }
        Ok(None)
    }

    /// Start a landing block: point the vector the trap came through back at
    /// its sink (and stvec too when landing in M, as nothing below M remains
    /// to use it), then read the cause and exception PC, which Spike knows,
    /// and the trap value, which is implementation-dependent.
    pub fn land(&mut self, bb: &mut BasicBlock, ctx: &mut Ctx, landing: Landing, rng: &mut (impl Rng + ?Sized)) {
        bb.landing = Some(landing.key);
        if self.vectored.contains(&landing.key) {
            bb.align = VECTORED_LANDING_ALIGN;
        }
        let (cause, epc, tval) = if landing.handler == M {
            ctx.point(bb, Csr::MTVEC, Dest::Landing { key: SINK_M, fallback: SINK_M }, false, rng);
            self.state.mtvec = None;
            if self.state.stvec.take().is_some() {
                ctx.point(bb, Csr::STVEC, Dest::Landing { key: SINK_S, fallback: SINK_S }, false, rng);
            }
            (Csr::MCAUSE, Csr::MEPC, Csr::MTVAL)
        } else {
            ctx.point(bb, Csr::STVEC, Dest::Landing { key: SINK_S, fallback: SINK_S }, false, rng);
            self.state.stvec = None;
            (Csr::SCAUSE, Csr::SEPC, Csr::STVAL)
        };
        ctx.read(bb, cause, true, rng);
        ctx.read(bb, epc, true, rng);
        if rng.random_bool(0.5) {
            ctx.read(bb, tval, false, rng);
        }
    }
}

/// A random `medeleg` within the target's mask.
fn random_medeleg(target: &Target, rng: &mut (impl Rng + ?Sized)) -> u64 {
    rng.random::<u64>() & target.medeleg_mask & !NEVER_DELEGATED
}

/// Random permissions for `count` window regions.
pub fn window_perms(count: usize, rng: &mut (impl Rng + ?Sized)) -> Vec<u8> {
    (0..count).map(|_| crate::csrs::pmp_cfg(rng) & 0b111).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csr_access_follows_the_mode() {
        let mut state = PrivState::default();
        for (mode, mscratch, sscratch) in [(M, true, true), (S, false, true), (U, false, false)] {
            state.mode = mode;
            assert_eq!(state.allows_csr(Csr::MSCRATCH), mscratch);
            assert_eq!(state.allows_csr(Csr::SSCRATCH), sscratch);
            assert!(state.allows_csr(Csr::FCSR));
        }
        state.fs_on = false;
        assert!(!state.allows_csr(Csr::FFLAGS));
        assert!(!state.allows(Opcode::FaddS));
        state.fs_on = true;
        state.mode = S;
        state.tvm = true;
        assert!(!state.allows(Opcode::SfenceVma));
        state.mode = U;
        state.tvm = false;
        assert!(!state.allows(Opcode::SfenceVma));
    }
}
