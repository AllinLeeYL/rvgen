// Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
// SPDX-License-Identifier: GPL-3.0-only

package riscv

type RegType uint8

const (
	RegTypeData     RegType = 1
	RegTypeAddress  RegType = 2
	RegTypeStandard RegType = 3
	RegTypeCsr      RegType = 4
	RegTypeFrm      RegType = 5
)

type AccCsr uint8

const (
	AccCsrNV AccCsr = 1
	AccCsrOF AccCsr = 2
	AccCsrUF AccCsr = 3
	AccCsrNX AccCsr = 4
	AccCsrDZ AccCsr = 5
)

type MemoryInstructionKind uint8

const (
	MemoryInstructionKindIntLoadInstr    MemoryInstructionKind = 0
	MemoryInstructionKindFloatLoadInstr  MemoryInstructionKind = 1
	MemoryInstructionKindIntStoreInstr   MemoryInstructionKind = 2
	MemoryInstructionKindFloatStoreInstr MemoryInstructionKind = 3
	MemoryInstructionKindAmoInstr        MemoryInstructionKind = 4
	MemoryInstructionKindAmoStore        MemoryInstructionKind = 5
)

type SyntacticDependency struct {
	Sources, Destinations []RegType
	AccumulatingCSRs      []AccCsr
	DependencyFlags       [2]bool
}

var LOAD_INSTR = []MemoryInstructionKind{
	MemoryInstructionKindIntLoadInstr,
	MemoryInstructionKindFloatLoadInstr,
}
var STORE_INSTR = []MemoryInstructionKind{
	MemoryInstructionKindIntStoreInstr,
	MemoryInstructionKindFloatStoreInstr,
}
var AMO_INSTR = []MemoryInstructionKind{
	MemoryInstructionKindAmoInstr,
	MemoryInstructionKindAmoStore,
}
var MEMOP = []MemoryInstructionKind{
	MemoryInstructionKindIntLoadInstr,
	MemoryInstructionKindFloatLoadInstr,
	MemoryInstructionKindIntStoreInstr,
	MemoryInstructionKindFloatStoreInstr,
	MemoryInstructionKindAmoInstr,
	MemoryInstructionKindAmoStore,
}
var FRM_FLOAT = []string{
	"fmadd.s",
	"fmsub.s",
	"fnmsub.s",
	"fnmadd.s",
	"fmadd.d",
	"fmsub.d",
	"fnmsub.d",
	"fnmadd.d",
	"fadd.s",
	"fsub.s",
	"fmul.s",
	"fdiv.s",
	"fadd.d",
	"fsub.d",
	"fmul.d",
	"fdiv.d",
	"fsqrt.s",
	"fsqrt.d",
	"fcvt.s.d",
	"fcvt.w.s",
	"fcvt.wu.s",
	"fcvt.l.s",
	"fcvt.lu.s",
	"fcvt.w.d",
	"fcvt.wu.d",
	"fcvt.l.d",
	"fcvt.lu.d",
	"fcvt.s.w",
	"fcvt.s.wu",
	"fcvt.s.l",
	"fcvt.s.lu",
	"fcvt.d.l",
	"fcvt.d.lu",
}
var syntacticDependencies = map[string]SyntacticDependency{
	"lui":       {nil, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"auipc":     {nil, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"jal":       {nil, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"jalr":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"beq":       {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"bne":       {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"blt":       {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"bge":       {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"bltu":      {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"bgeu":      {[]RegType{RegTypeStandard, RegTypeStandard}, nil, nil, [2]bool{false, false}},
	"lb":        {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"lh":        {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"lw":        {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"lbu":       {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"lhu":       {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"sb":        {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"sh":        {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"sw":        {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"addi":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"slti":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sltiu":     {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"xori":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"ori":       {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"andi":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"slli":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"srli":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"srai":      {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"add":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sub":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sll":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"slt":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sltu":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"xor":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"srl":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sra":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"or":        {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"and":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fence":     {nil, nil, nil, [2]bool{false, false}},
	"fence.i":   {nil, nil, nil, [2]bool{false, false}},
	"ecall":     {nil, nil, nil, [2]bool{false, false}},
	"ebreak":    {nil, nil, nil, [2]bool{false, false}},
	"csrrw":     {[]RegType{RegTypeStandard, RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"csrrs":     {[]RegType{RegTypeStandard, RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"csrrc":     {[]RegType{RegTypeStandard, RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"csrrwi":    {[]RegType{RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"csrrsi":    {[]RegType{RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"csrrci":    {[]RegType{RegTypeCsr}, []RegType{RegTypeStandard, RegTypeCsr}, nil, [2]bool{true, true}},
	"lwu":       {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"ld":        {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"sd":        {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"addiw":     {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"slliw":     {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"srliw":     {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sraiw":     {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"addw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"subw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sllw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"srlw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"sraw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"mul":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"mulh":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"mulhsu":    {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"mulhu":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"div":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"divu":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"rem":       {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"remu":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"mulw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"divw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"divuw":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"remw":      {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"remuw":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"lr.w":      {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"sc.w":      {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{true, false}},
	"amoswap.w": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoadd.w":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoxor.w":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoand.w":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoor.w":   {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomin.w":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomax.w":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amominu.w": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomaxu.w": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"lr.d":      {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"sc.d":      {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{true, false}},
	"amoswap.d": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoadd.d":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoxor.d":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoand.d":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amoor.d":   {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomin.d":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomax.d":  {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amominu.d": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"amomaxu.d": {[]RegType{RegTypeAddress, RegTypeData}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"flw":       {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"fsw":       {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"fmadd.s": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fmsub.s": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fnmsub.s": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fnmadd.s": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fadd.s":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrNX}, [2]bool{true, true}},
	"fsub.s":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrNX}, [2]bool{true, true}},
	"fmul.s":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fdiv.s":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrDZ, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fsqrt.s":   {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fsgnj.s":   {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fsgnjn.s":  {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fsgnjx.s":  {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fmin.s":    {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fmax.s":    {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fcvt.w.s":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.wu.s": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fmv.x.w":   {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"feq.s":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"flt.s":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fle.s":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fclass.s":  {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.s.w":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fcvt.s.wu": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fmv.w.x":   {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.l.s":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.lu.s": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.s.l":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fcvt.s.lu": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fld":       {[]RegType{RegTypeAddress}, []RegType{RegTypeStandard}, nil, [2]bool{false, false}},
	"fsd":       {[]RegType{RegTypeAddress, RegTypeData}, nil, nil, [2]bool{false, false}},
	"fmadd.d": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fmsub.d": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fnmsub.d": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fnmadd.d": {[]RegType{
		RegTypeStandard,
		RegTypeStandard,
		RegTypeStandard,
		RegTypeFrm,
	}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fadd.d":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrNX}, [2]bool{true, true}},
	"fsub.d":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrNX}, [2]bool{true, true}},
	"fmul.d":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fdiv.d":    {[]RegType{RegTypeStandard, RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrDZ, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fsqrt.d":   {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fsgnj.d":   {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fsgnjn.d":  {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fsgnjx.d":  {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fmin.d":    {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fmax.d":    {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fcvt.s.d":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrOF, AccCsrUF, AccCsrNX}, [2]bool{true, true}},
	"fcvt.d.s":  {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"feq.d":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"flt.d":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fle.d":     {[]RegType{RegTypeStandard, RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV}, [2]bool{false, true}},
	"fclass.d":  {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.w.d":  {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.wu.d": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.d.w":  {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.d.wu": {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.l.d":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fcvt.lu.d": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNV, AccCsrNX}, [2]bool{true, true}},
	"fmv.x.d":   {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
	"fcvt.d.l":  {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fcvt.d.lu": {[]RegType{RegTypeStandard, RegTypeFrm}, []RegType{RegTypeStandard}, []AccCsr{AccCsrNX}, [2]bool{true, true}},
	"fmv.d.x":   {[]RegType{RegTypeStandard}, []RegType{RegTypeStandard}, nil, [2]bool{false, true}},
}

func SyntacticDependencyFor(mnemonic string) (SyntacticDependency, bool) {
	d, ok := syntacticDependencies[mnemonic]
	return d, ok
}
