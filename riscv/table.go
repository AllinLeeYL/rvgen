package riscv

// InstructionClass identifies a family available to weighted generation.
type InstructionClass uint8

const (
	NoClass InstructionClass = iota
	ClassAlu
	ClassAlu64
	ClassMulDiv
	ClassMulDiv64
	ClassMemory
	ClassMemory64
	ClassBranch
	ClassJal
	ClassJalr
	ClassAmo
	ClassAmo64
	ClassFloatMemory
	ClassFloat
	ClassFloat64
	ClassDoubleMemory
	ClassDouble
	ClassDouble64
	ClassFence
	ClassCsr
)

func (c InstructionClass) RequiresRV64() bool {
	switch c {
	case ClassAlu64, ClassMulDiv64, ClassMemory64, ClassAmo64, ClassFloat64, ClassDouble64:
		return true
	}
	return false
}

type InstructionKind uint16

const (
	Raw32 InstructionKind = iota
	Raw16
	Lui
	Auipc
	Jal
	Jalr
	Beq
	Bne
	Blt
	Bge
	Bltu
	Bgeu
	Lb
	Lh
	Lw
	Lbu
	Lhu
	Sb
	Sh
	Sw
	Addi
	Slti
	Sltiu
	Xori
	Ori
	Andi
	Slli
	Srli
	Srai
	Add
	Sub
	Sll
	Slt
	Sltu
	Xor
	Srl
	Sra
	Or
	And
	Fence
	Ecall
	Ebreak
	Lwu
	Ld
	Sd
	Addiw
	Slliw
	Srliw
	Sraiw
	Addw
	Subw
	Sllw
	Srlw
	Sraw
	Mul
	Mulh
	Mulhsu
	Mulhu
	Div
	Divu
	Rem
	Remu
	Mulw
	Divw
	Divuw
	Remw
	Remuw
	LrW
	ScW
	AmoswapW
	AmoaddW
	AmoandW
	AmoorW
	AmoxorW
	AmomaxW
	AmomaxuW
	AmominW
	AmominuW
	LrD
	ScD
	AmoswapD
	AmoaddD
	AmoandD
	AmoorD
	AmoxorD
	AmomaxD
	AmomaxuD
	AmominD
	AmominuD
	Flw
	Fsw
	FmaddS
	FmsubS
	FnmsubS
	FnmaddS
	FaddS
	FsubS
	FmulS
	FdivS
	FsqrtS
	FsgnjS
	FsgnjnS
	FsgnjxS
	FminS
	FmaxS
	FcvtWS
	FcvtWuS
	FmvXW
	FeqS
	FltS
	FleS
	FclassS
	FcvtSW
	FcvtSWu
	FmvWX
	FcvtLS
	FcvtLuS
	FcvtSL
	FcvtSLu
	Fld
	Fsd
	FmaddD
	FmsubD
	FnmsubD
	FnmaddD
	FaddD
	FsubD
	FmulD
	FdivD
	FsqrtD
	FsgnjD
	FsgnjnD
	FsgnjxD
	FminD
	FmaxD
	FcvtSD
	FcvtDS
	FeqD
	FltD
	FleD
	FclassD
	FcvtWD
	FcvtWuD
	FcvtDW
	FcvtDWu
	FcvtLD
	FcvtLuD
	FmvXD
	FcvtDL
	FcvtDLu
	FmvDX
	CMv
	CAdd
	CAnd
	COr
	CXor
	CSub
	CLui
	CAddi16sp
	CAddi4spn
	CAddi
	CLi
	CSlli
	CAndi
	CSrli
	CSrai
	CBeqz
	CBnez
	CJal
	CJ
	CJr
	CJalr
	CEbreak
	CLwsp
	CLw
	CSwsp
	CSw
	CAddw
	CSubw
	CAddiw
	CLdsp
	CLd
	CSdsp
	CSd
	Csrrw
	Csrrs
	Csrrc
	Csrrwi
	Csrrsi
	Csrrci
	FenceI
	Sret
	Mret
	Wfi
	SfenceVma
	SinvalVma
	SfenceWInval
	SfenceInvalIr
)

type operandField uint8

const (
	fieldRd operandField = iota
	fieldRs1
	fieldRs2
	fieldRs3
	fieldImm
	fieldShamt
	fieldRm
	fieldCsr
	fieldUimm
	fieldAq
	fieldRl
)

type operandSpec struct {
	field    operandField
	floating bool
}
type instructionInfo struct {
	mnemonic string
	class    InstructionClass
	width    int
	operands []operandSpec
	encode   func(Instruction) uint32
}

var instructionTable = [...]instructionInfo{
	Raw32:         {width: 4, encode: func(i Instruction) uint32 { return i.Raw }},
	Raw16:         {width: 2, encode: func(i Instruction) uint32 { return i.Raw }},
	Lui:           {"lui", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lui(i.Rd, i.Imm) }},
	Auipc:         {"auipc", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_auipc(i.Rd, i.Imm) }},
	Jal:           {"jal", ClassJal, 4, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_jal(i.Rd, i.Imm) }},
	Jalr:          {"jalr", ClassJalr, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_jalr(i.Rd, i.Rs1, i.Imm) }},
	Beq:           {"beq", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_beq(i.Rs1, i.Rs2, i.Imm) }},
	Bne:           {"bne", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_bne(i.Rs1, i.Rs2, i.Imm) }},
	Blt:           {"blt", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_blt(i.Rs1, i.Rs2, i.Imm) }},
	Bge:           {"bge", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_bge(i.Rs1, i.Rs2, i.Imm) }},
	Bltu:          {"bltu", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_bltu(i.Rs1, i.Rs2, i.Imm) }},
	Bgeu:          {"bgeu", ClassBranch, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_bgeu(i.Rs1, i.Rs2, i.Imm) }},
	Lb:            {"lb", ClassMemory, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lb(i.Rd, i.Rs1, i.Imm) }},
	Lh:            {"lh", ClassMemory, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lh(i.Rd, i.Rs1, i.Imm) }},
	Lw:            {"lw", ClassMemory, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lw(i.Rd, i.Rs1, i.Imm) }},
	Lbu:           {"lbu", ClassMemory, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lbu(i.Rd, i.Rs1, i.Imm) }},
	Lhu:           {"lhu", ClassMemory, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_lhu(i.Rd, i.Rs1, i.Imm) }},
	Sb:            {"sb", ClassMemory, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_sb(i.Rs1, i.Rs2, i.Imm) }},
	Sh:            {"sh", ClassMemory, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_sh(i.Rs1, i.Rs2, i.Imm) }},
	Sw:            {"sw", ClassMemory, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_sw(i.Rs1, i.Rs2, i.Imm) }},
	Addi:          {"addi", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_addi(i.Rd, i.Rs1, i.Imm) }},
	Slti:          {"slti", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_slti(i.Rd, i.Rs1, i.Imm) }},
	Sltiu:         {"sltiu", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_sltiu(i.Rd, i.Rs1, i.Imm) }},
	Xori:          {"xori", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_xori(i.Rd, i.Rs1, i.Imm) }},
	Ori:           {"ori", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_ori(i.Rd, i.Rs1, i.Imm) }},
	Andi:          {"andi", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_andi(i.Rd, i.Rs1, i.Imm) }},
	Slli:          {"slli", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv32i_slli(i.Rd, i.Rs1, i.Shamt) }},
	Srli:          {"srli", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv32i_srli(i.Rd, i.Rs1, i.Shamt) }},
	Srai:          {"srai", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv32i_srai(i.Rd, i.Rs1, i.Shamt) }},
	Add:           {"add", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_add(i.Rd, i.Rs1, i.Rs2) }},
	Sub:           {"sub", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_sub(i.Rd, i.Rs1, i.Rs2) }},
	Sll:           {"sll", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_sll(i.Rd, i.Rs1, i.Rs2) }},
	Slt:           {"slt", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_slt(i.Rd, i.Rs1, i.Rs2) }},
	Sltu:          {"sltu", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_sltu(i.Rd, i.Rs1, i.Rs2) }},
	Xor:           {"xor", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_xor(i.Rd, i.Rs1, i.Rs2) }},
	Srl:           {"srl", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_srl(i.Rd, i.Rs1, i.Rs2) }},
	Sra:           {"sra", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_sra(i.Rd, i.Rs1, i.Rs2) }},
	Or:            {"or", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_or(i.Rd, i.Rs1, i.Rs2) }},
	And:           {"and", ClassAlu, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32i_and(i.Rd, i.Rs1, i.Rs2) }},
	Fence:         {"fence", ClassFence, 4, []operandSpec{{fieldImm, false}}, func(i Instruction) uint32 { return Rv32i_fence(i.Imm) }},
	Ecall:         {"ecall", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rv32i_ecall() }},
	Ebreak:        {"ebreak", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rv32i_ebreak() }},
	Lwu:           {"lwu", ClassMemory64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64i_lwu(i.Rd, i.Rs1, i.Imm) }},
	Ld:            {"ld", ClassMemory64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64i_ld(i.Rd, i.Rs1, i.Imm) }},
	Sd:            {"sd", ClassMemory64, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64i_sd(i.Rs1, i.Rs2, i.Imm) }},
	Addiw:         {"addiw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64i_addiw(i.Rd, i.Rs1, i.Imm) }},
	Slliw:         {"slliw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv64i_slliw(i.Rd, i.Rs1, i.Shamt) }},
	Srliw:         {"srliw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv64i_srliw(i.Rd, i.Rs1, i.Shamt) }},
	Sraiw:         {"sraiw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldShamt, false}}, func(i Instruction) uint32 { return Rv64i_sraiw(i.Rd, i.Rs1, i.Shamt) }},
	Addw:          {"addw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64i_addw(i.Rd, i.Rs1, i.Rs2) }},
	Subw:          {"subw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64i_subw(i.Rd, i.Rs1, i.Rs2) }},
	Sllw:          {"sllw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64i_sllw(i.Rd, i.Rs1, i.Rs2) }},
	Srlw:          {"srlw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64i_srlw(i.Rd, i.Rs1, i.Rs2) }},
	Sraw:          {"sraw", ClassAlu64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64i_sraw(i.Rd, i.Rs1, i.Rs2) }},
	Mul:           {"mul", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_mul(i.Rd, i.Rs1, i.Rs2) }},
	Mulh:          {"mulh", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_mulh(i.Rd, i.Rs1, i.Rs2) }},
	Mulhsu:        {"mulhsu", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_mulhsu(i.Rd, i.Rs1, i.Rs2) }},
	Mulhu:         {"mulhu", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_mulhu(i.Rd, i.Rs1, i.Rs2) }},
	Div:           {"div", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_div(i.Rd, i.Rs1, i.Rs2) }},
	Divu:          {"divu", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_divu(i.Rd, i.Rs1, i.Rs2) }},
	Rem:           {"rem", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_rem(i.Rd, i.Rs1, i.Rs2) }},
	Remu:          {"remu", ClassMulDiv, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32m_remu(i.Rd, i.Rs1, i.Rs2) }},
	Mulw:          {"mulw", ClassMulDiv64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64m_mulw(i.Rd, i.Rs1, i.Rs2) }},
	Divw:          {"divw", ClassMulDiv64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64m_divw(i.Rd, i.Rs1, i.Rs2) }},
	Divuw:         {"divuw", ClassMulDiv64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64m_divuw(i.Rd, i.Rs1, i.Rs2) }},
	Remw:          {"remw", ClassMulDiv64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64m_remw(i.Rd, i.Rs1, i.Rs2) }},
	Remuw:         {"remuw", ClassMulDiv64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64m_remuw(i.Rd, i.Rs1, i.Rs2) }},
	LrW:           {"lr.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}}, func(i Instruction) uint32 { return Rv32a_lrw(i.Aq, i.Rl, i.Rd, i.Rs1, 0) }},
	ScW:           {"sc.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_scw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoswapW:      {"amoswap.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amoswapw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoaddW:       {"amoadd.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amoaddw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoandW:       {"amoand.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amoandw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoorW:        {"amoor.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amoorw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoxorW:       {"amoxor.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amoxorw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmomaxW:       {"amomax.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amomaxw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmomaxuW:      {"amomaxu.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amomaxuw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmominW:       {"amomin.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amominw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmominuW:      {"amominu.w", ClassAmo, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32a_amominuw(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	LrD:           {"lr.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}}, func(i Instruction) uint32 { return Rv64a_lrd(i.Aq, i.Rl, i.Rd, i.Rs1, 0) }},
	ScD:           {"sc.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_scd(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoswapD:      {"amoswap.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amoswapd(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoaddD:       {"amoadd.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amoaddd(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoandD:       {"amoand.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amoandd(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoorD:        {"amoor.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amoord(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmoxorD:       {"amoxor.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amoxord(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmomaxD:       {"amomax.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amomaxd(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmomaxuD:      {"amomaxu.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amomaxud(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmominD:       {"amomin.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amomind(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	AmominuD:      {"amominu.d", ClassAmo64, 4, []operandSpec{{fieldAq, false}, {fieldRl, false}, {fieldRd, false}, {fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64a_amominud(i.Aq, i.Rl, i.Rd, i.Rs1, i.Rs2) }},
	Flw:           {"flw", ClassFloatMemory, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32f_flw(i.Rd, i.Rs1, i.Imm) }},
	Fsw:           {"fsw", ClassFloatMemory, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, true}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32f_fsw(i.Rs1, i.Rs2, i.Imm) }},
	FmaddS:        {"fmadd.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fmadds(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FmsubS:        {"fmsub.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fmsubs(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FnmsubS:       {"fnmsub.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fnmsubs(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FnmaddS:       {"fnmadd.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fnmadds(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FaddS:         {"fadd.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fadds(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FsubS:         {"fsub.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fsubs(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FmulS:         {"fmul.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fmuls(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FdivS:         {"fdiv.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fdivs(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FsqrtS:        {"fsqrt.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fsqrts(i.Rd, i.Rs1, i.Rm) }},
	FsgnjS:        {"fsgnj.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fsgnjs(i.Rd, i.Rs1, i.Rs2) }},
	FsgnjnS:       {"fsgnjn.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fsgnjns(i.Rd, i.Rs1, i.Rs2) }},
	FsgnjxS:       {"fsgnjx.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fsgnjxs(i.Rd, i.Rs1, i.Rs2) }},
	FminS:         {"fmin.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fmins(i.Rd, i.Rs1, i.Rs2) }},
	FmaxS:         {"fmax.s", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fmaxs(i.Rd, i.Rs1, i.Rs2) }},
	FcvtWS:        {"fcvt.w.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fcvtws(i.Rd, i.Rs1, i.Rm) }},
	FcvtWuS:       {"fcvt.wu.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fcvtwus(i.Rd, i.Rs1, i.Rm) }},
	FmvXW:         {"fmv.x.w", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}}, func(i Instruction) uint32 { return Rv32f_fmvxw(i.Rd, i.Rs1) }},
	FeqS:          {"feq.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_feqs(i.Rd, i.Rs1, i.Rs2) }},
	FltS:          {"flt.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_flts(i.Rd, i.Rs1, i.Rs2) }},
	FleS:          {"fle.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32f_fles(i.Rd, i.Rs1, i.Rs2) }},
	FclassS:       {"fclass.s", ClassFloat, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}}, func(i Instruction) uint32 { return Rv32f_fclasss(i.Rd, i.Rs1) }},
	FcvtSW:        {"fcvt.s.w", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fcvtsw(i.Rd, i.Rs1, i.Rm) }},
	FcvtSWu:       {"fcvt.s.wu", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32f_fcvtswu(i.Rd, i.Rs1, i.Rm) }},
	FmvWX:         {"fmv.w.x", ClassFloat, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}}, func(i Instruction) uint32 { return Rv32f_fmvwx(i.Rd, i.Rs1) }},
	FcvtLS:        {"fcvt.l.s", ClassFloat64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64f_fcvtls(i.Rd, i.Rs1, i.Rm) }},
	FcvtLuS:       {"fcvt.lu.s", ClassFloat64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64f_fcvtlus(i.Rd, i.Rs1, i.Rm) }},
	FcvtSL:        {"fcvt.s.l", ClassFloat64, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64f_fcvtsl(i.Rd, i.Rs1, i.Rm) }},
	FcvtSLu:       {"fcvt.s.lu", ClassFloat64, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64f_fcvtslu(i.Rd, i.Rs1, i.Rm) }},
	Fld:           {"fld", ClassDoubleMemory, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32d_fld(i.Rd, i.Rs1, i.Imm) }},
	Fsd:           {"fsd", ClassDoubleMemory, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, true}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32d_fsd(i.Rs1, i.Rs2, i.Imm) }},
	FmaddD:        {"fmadd.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fmaddd(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FmsubD:        {"fmsub.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fmsubd(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FnmsubD:       {"fnmsub.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fnmsubd(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FnmaddD:       {"fnmadd.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRs3, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fnmaddd(i.Rd, i.Rs1, i.Rs2, i.Rs3, i.Rm) }},
	FaddD:         {"fadd.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_faddd(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FsubD:         {"fsub.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fsubd(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FmulD:         {"fmul.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fmuld(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FdivD:         {"fdiv.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fdivd(i.Rd, i.Rs1, i.Rs2, i.Rm) }},
	FsqrtD:        {"fsqrt.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fsqrtd(i.Rd, i.Rs1, i.Rm) }},
	FsgnjD:        {"fsgnj.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fsgnjd(i.Rd, i.Rs1, i.Rs2) }},
	FsgnjnD:       {"fsgnjn.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fsgnjnd(i.Rd, i.Rs1, i.Rs2) }},
	FsgnjxD:       {"fsgnjx.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fsgnjxd(i.Rd, i.Rs1, i.Rs2) }},
	FminD:         {"fmin.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fmind(i.Rd, i.Rs1, i.Rs2) }},
	FmaxD:         {"fmax.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fmaxd(i.Rd, i.Rs1, i.Rs2) }},
	FcvtSD:        {"fcvt.s.d", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtsd(i.Rd, i.Rs1, i.Rm) }},
	FcvtDS:        {"fcvt.d.s", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtds(i.Rd, i.Rs1, i.Rm) }},
	FeqD:          {"feq.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_feqd(i.Rd, i.Rs1, i.Rs2) }},
	FltD:          {"flt.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fltd(i.Rd, i.Rs1, i.Rs2) }},
	FleD:          {"fle.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRs2, true}}, func(i Instruction) uint32 { return Rv32d_fled(i.Rd, i.Rs1, i.Rs2) }},
	FclassD:       {"fclass.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}}, func(i Instruction) uint32 { return Rv32d_fclassd(i.Rd, i.Rs1) }},
	FcvtWD:        {"fcvt.w.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtwd(i.Rd, i.Rs1, i.Rm) }},
	FcvtWuD:       {"fcvt.wu.d", ClassDouble, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtwud(i.Rd, i.Rs1, i.Rm) }},
	FcvtDW:        {"fcvt.d.w", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtdw(i.Rd, i.Rs1, i.Rm) }},
	FcvtDWu:       {"fcvt.d.wu", ClassDouble, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv32d_fcvtdwu(i.Rd, i.Rs1, i.Rm) }},
	FcvtLD:        {"fcvt.l.d", ClassDouble64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64d_fcvtld(i.Rd, i.Rs1, i.Rm) }},
	FcvtLuD:       {"fcvt.lu.d", ClassDouble64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64d_fcvtlud(i.Rd, i.Rs1, i.Rm) }},
	FmvXD:         {"fmv.x.d", ClassDouble64, 4, []operandSpec{{fieldRd, false}, {fieldRs1, true}}, func(i Instruction) uint32 { return Rv64d_fmvxd(i.Rd, i.Rs1) }},
	FcvtDL:        {"fcvt.d.l", ClassDouble64, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64d_fcvtdl(i.Rd, i.Rs1, i.Rm) }},
	FcvtDLu:       {"fcvt.d.lu", ClassDouble64, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}, {fieldRm, false}}, func(i Instruction) uint32 { return Rv64d_fcvtdlu(i.Rd, i.Rs1, i.Rm) }},
	FmvDX:         {"fmv.d.x", ClassDouble64, 4, []operandSpec{{fieldRd, true}, {fieldRs1, false}}, func(i Instruction) uint32 { return Rv64d_fmvdx(i.Rd, i.Rs1) }},
	CMv:           {"c.mv", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_mv(i.Rd, i.Rs2) }},
	CAdd:          {"c.add", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_add(i.Rd, i.Rs2) }},
	CAnd:          {"c.and", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_and(i.Rd, i.Rs2) }},
	COr:           {"c.or", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_or(i.Rd, i.Rs2) }},
	CXor:          {"c.xor", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_xor(i.Rd, i.Rs2) }},
	CSub:          {"c.sub", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv32ic_sub(i.Rd, i.Rs2) }},
	CLui:          {"c.lui", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_lui(i.Rd, i.Imm) }},
	CAddi16sp:     {"c.addi16sp", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_addi16sp(i.Rd, i.Imm) }},
	CAddi4spn:     {"c.addi4spn", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_addi4spn(i.Rd, i.Imm) }},
	CAddi:         {"c.addi", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_addi(i.Rd, i.Imm) }},
	CLi:           {"c.li", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_li(i.Rd, i.Imm) }},
	CSlli:         {"c.slli", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_slli(i.Rd, i.Imm) }},
	CAndi:         {"c.andi", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_andi(i.Rs1, i.Imm) }},
	CSrli:         {"c.srli", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_srli(i.Rs1, i.Imm) }},
	CSrai:         {"c.srai", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_srai(i.Rs1, i.Imm) }},
	CBeqz:         {"c.beqz", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_beqz(i.Rs1, i.Imm) }},
	CBnez:         {"c.bnez", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_bnez(i.Rs1, i.Imm) }},
	CJal:          {"c.jal", NoClass, 2, []operandSpec{{fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_jal(i.Imm) }},
	CJ:            {"c.j", NoClass, 2, []operandSpec{{fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_j(i.Imm) }},
	CJr:           {"c.jr", NoClass, 2, []operandSpec{{fieldRs1, false}}, func(i Instruction) uint32 { return Rv32ic_jr(i.Rs1) }},
	CJalr:         {"c.jalr", NoClass, 2, []operandSpec{{fieldRs1, false}}, func(i Instruction) uint32 { return Rv32ic_jalr(i.Rs1) }},
	CEbreak:       {"c.ebreak", NoClass, 2, []operandSpec{}, func(i Instruction) uint32 { return Rv32ic_ebreak() }},
	CLwsp:         {"c.lwsp", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_lwsp(i.Rd, i.Imm) }},
	CLw:           {"c.lw", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_lw(i.Rd, i.Rs1, i.Imm) }},
	CSwsp:         {"c.swsp", NoClass, 2, []operandSpec{{fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_swsp(i.Rs2, i.Imm) }},
	CSw:           {"c.sw", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv32ic_sw(i.Rs1, i.Rs2, i.Imm) }},
	CAddw:         {"c.addw", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64ic_addw(i.Rd, i.Rs2) }},
	CSubw:         {"c.subw", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rv64ic_subw(i.Rd, i.Rs2) }},
	CAddiw:        {"c.addiw", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64ic_addiw(i.Rd, i.Imm) }},
	CLdsp:         {"c.ldsp", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64ic_ldsp(i.Rd, i.Imm) }},
	CLd:           {"c.ld", NoClass, 2, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64ic_ld(i.Rd, i.Rs1, i.Imm) }},
	CSdsp:         {"c.sdsp", NoClass, 2, []operandSpec{{fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64ic_sdsp(i.Rs2, i.Imm) }},
	CSd:           {"c.sd", NoClass, 2, []operandSpec{{fieldRs1, false}, {fieldRs2, false}, {fieldImm, false}}, func(i Instruction) uint32 { return Rv64ic_sd(i.Rs1, i.Rs2, i.Imm) }},
	Csrrw:         {"csrrw", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrw(i.Rd, i.Rs1, i.Csr) }},
	Csrrs:         {"csrrs", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrs(i.Rd, i.Rs1, i.Csr) }},
	Csrrc:         {"csrrc", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldRs1, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrc(i.Rd, i.Rs1, i.Csr) }},
	Csrrwi:        {"csrrwi", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldUimm, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrwi(i.Rd, i.Uimm, i.Csr) }},
	Csrrsi:        {"csrrsi", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldUimm, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrsi(i.Rd, i.Uimm, i.Csr) }},
	Csrrci:        {"csrrci", ClassCsr, 4, []operandSpec{{fieldRd, false}, {fieldUimm, false}, {fieldCsr, false}}, func(i Instruction) uint32 { return Zicsr_csrrci(i.Rd, i.Uimm, i.Csr) }},
	FenceI:        {"fence.i", ClassFence, 4, []operandSpec{{fieldImm, false}}, func(i Instruction) uint32 { return Zifencei_fencei(i.Imm) }},
	Sret:          {"sret", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rvprivileged_sret() }},
	Mret:          {"mret", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rvprivileged_mret() }},
	Wfi:           {"wfi", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rvprivileged_wfi() }},
	SfenceVma:     {"sfence.vma", NoClass, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rvprivileged_sfence_vma(i.Rs1, i.Rs2) }},
	SinvalVma:     {"sinval.vma", NoClass, 4, []operandSpec{{fieldRs1, false}, {fieldRs2, false}}, func(i Instruction) uint32 { return Rvprivileged_sinval_vma(i.Rs1, i.Rs2) }},
	SfenceWInval:  {"sfence.w.inval", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rvprivileged_sfence_w_inval() }},
	SfenceInvalIr: {"sfence.inval.ir", NoClass, 4, []operandSpec{}, func(i Instruction) uint32 { return Rvprivileged_sfence_inval_ir() }},
}

func AllKinds() []InstructionKind {
	kinds := make([]InstructionKind, 0, len(instructionTable)-2)
	for k := InstructionKind(2); int(k) < len(instructionTable); k++ {
		kinds = append(kinds, k)
	}
	return kinds
}
func (k InstructionKind) Class() InstructionClass { return instructionTable[k].class }
func (k InstructionKind) Mnemonic() string        { return instructionTable[k].mnemonic }
func (c InstructionClass) Kinds() []InstructionKind {
	var kinds []InstructionKind
	for _, k := range AllKinds() {
		if k.Class() == c {
			kinds = append(kinds, k)
		}
	}
	return kinds
}
