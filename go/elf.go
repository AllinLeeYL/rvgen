package rvgen

import (
	"encoding/binary"
	"fmt"
	"math"
)

const (
	SHFWrite     uint64 = 1
	SHFAlloc     uint64 = 2
	SHFExecInstr uint64 = 4
	// The writer holds the complete image in memory. Reject unreasonable layouts
	// (including huge alignment gaps) before allocating their backing storage.
	MaxELFSize uint64 = 1 << 30
)

type ELFSection struct {
	Name               string
	Bytes              []byte
	Addr, Flags, Align uint64
	Type, Link, Info   uint32
	Entsize            uint64
}

func NewELFSection(name string, data []byte) ELFSection {
	return ELFSection{Name: name, Bytes: data, Flags: SHFAlloc | SHFExecInstr, Align: 4, Type: 1}
}

// Section is a zero-based index; symbols are global and defined within a section.
type ELFSymbol struct {
	Name         string
	Section      int
	Offset, Size uint64
	Type         uint8
}

func add64(a, b uint64) (uint64, error) {
	if b > math.MaxUint64-a {
		return 0, fmt.Errorf("ELF layout exceeds 64 bits")
	}
	return a + b, nil
}
func alignUp(n, alignment uint64) (uint64, error) {
	if alignment == 0 {
		alignment = 1
	}
	return add64(n, (alignment-n%alignment)%alignment)
}
func validELFName(s string) bool {
	if s == "" {
		return false
	}
	for i := range s {
		if s[i] == 0 || s[i] >= 128 {
			return false
		}
	}
	return true
}
func SectionFlagsToProgramFlags(flags uint64) uint32 {
	p := uint32(4)
	if flags&SHFWrite != 0 {
		p |= 2
	}
	if flags&SHFExecInstr != 0 {
		p |= 1
	}
	return p
}

type elfBytes struct {
	data []byte
	is64 bool
	err  error
}

func (b *elfBytes) u16(v uint16) { b.data = binary.LittleEndian.AppendUint16(b.data, v) }
func (b *elfBytes) u32(v uint32) { b.data = binary.LittleEndian.AppendUint32(b.data, v) }
func (b *elfBytes) u64(v uint64) { b.data = binary.LittleEndian.AppendUint64(b.data, v) }
func (b *elfBytes) word(v uint64) {
	if b.is64 {
		b.u64(v)
	} else {
		if v > math.MaxUint32 {
			b.err = fmt.Errorf("value does not fit ELF32")
		}
		b.u32(uint32(v))
	}
}
func (b *elfBytes) pad(n uint64) { b.data = append(b.data, make([]byte, int(n)-len(b.data))...) }

// BuildELFWithSymbols adds symbol and string tables before laying out the ELF.
func BuildELFWithSymbols(sections []ELFSection, symbols []ELFSymbol, is64 bool, start uint64) ([]byte, error) {
	sections = append([]ELFSection(nil), sections...)
	strings := []byte{0}
	entrySize := 16
	if is64 {
		entrySize = 24
	}
	table := elfBytes{data: make([]byte, entrySize), is64: is64}
	names := map[string]bool{}
	for _, s := range symbols {
		if !validELFName(s.Name) || names[s.Name] || s.Type > 15 {
			return nil, fmt.Errorf("invalid or duplicate ELF symbol")
		}
		names[s.Name] = true
		if s.Section < 0 || s.Section >= len(sections) || s.Section+1 > math.MaxUint16 {
			return nil, fmt.Errorf("invalid symbol section")
		}
		section := sections[s.Section]
		end, err := add64(s.Offset, s.Size)
		if err != nil || end > uint64(len(section.Bytes)) {
			return nil, fmt.Errorf("symbol lies outside its section")
		}
		value, err := add64(section.Addr, s.Offset)
		if err != nil {
			return nil, err
		}
		if uint64(len(strings)) > math.MaxUint32 {
			return nil, fmt.Errorf("symbol string table too large")
		}
		table.u32(uint32(len(strings)))
		strings = append(strings, s.Name...)
		strings = append(strings, 0)
		if is64 {
			table.data = append(table.data, 0x10|s.Type, 0)
			table.u16(uint16(s.Section + 1))
			table.u64(value)
			table.u64(s.Size)
		} else {
			table.word(value)
			table.word(s.Size)
			table.data = append(table.data, 0x10|s.Type, 0)
			table.u16(uint16(s.Section + 1))
		}
	}
	if table.err != nil {
		return nil, table.err
	}
	stringIndex := uint32(len(sections) + 1)
	strtab := ELFSection{Name: ".strtab", Bytes: strings, Align: 1, Type: 3}
	symtab := ELFSection{Name: ".symtab", Bytes: table.data, Align: 4, Type: 2, Link: stringIndex, Info: 1, Entsize: uint64(entrySize)}
	if is64 {
		symtab.Align = 8
	}
	return BuildELF(append(sections, strtab, symtab), is64, start)
}

// BuildELF lays out sections and load segments in an ELF32 or ELF64 image.
func BuildELF(sections []ELFSection, is64 bool, start uint64) ([]byte, error) {
	if len(sections)+2 >= 0xff00 {
		return nil, fmt.Errorf("too many ELF sections")
	}
	ehsize, phsize, shsize := uint64(52), uint64(32), uint64(40)
	if is64 {
		ehsize, phsize, shsize = 64, 56, 64
	}
	phnum := 0
	for _, s := range sections {
		if s.Flags&SHFAlloc != 0 {
			phnum++
		}
	}
	names := map[string]bool{}
	shstrtab := []byte{0}
	nameOffsets := make([]uint32, 0, len(sections)+1)
	offsets := make([]uint64, 0, len(sections)+1)
	offset := ehsize + uint64(phnum)*phsize
	for _, s := range sections {
		if !validELFName(s.Name) || s.Name == ".shstrtab" || names[s.Name] {
			return nil, fmt.Errorf("invalid or duplicate section name: %q", s.Name)
		}
		names[s.Name] = true
		if s.Align > 1 && s.Align&(s.Align-1) != 0 {
			return nil, fmt.Errorf("section %s alignment must be a power of two", s.Name)
		}
		end, err := add64(s.Addr, uint64(len(s.Bytes)))
		if err != nil {
			return nil, err
		}
		if !is64 && (s.Addr > math.MaxUint32 || end > 1<<32) {
			return nil, fmt.Errorf("section address does not fit ELF32")
		}
		if uint64(len(shstrtab)) > math.MaxUint32 {
			return nil, fmt.Errorf("section name table too large")
		}
		nameOffsets = append(nameOffsets, uint32(len(shstrtab)))
		shstrtab = append(shstrtab, s.Name...)
		shstrtab = append(shstrtab, 0)
		offset, err = alignUp(offset, s.Align)
		if err != nil {
			return nil, err
		}
		offsets = append(offsets, offset)
		offset, err = add64(offset, uint64(len(s.Bytes)))
		if err != nil {
			return nil, err
		}
	}
	nameOffsets = append(nameOffsets, uint32(len(shstrtab)))
	shstrtab = append(shstrtab, ".shstrtab\x00"...)
	offsets = append(offsets, offset)
	shoff, err := add64(offset, uint64(len(shstrtab)))
	if err != nil {
		return nil, err
	}
	shoff, err = alignUp(shoff, 8)
	if err != nil {
		return nil, err
	}
	fileSize, err := add64(shoff, uint64(len(sections)+2)*shsize)
	if err != nil {
		return nil, err
	}
	if !is64 && (fileSize > math.MaxUint32 || start > math.MaxUint32) {
		return nil, fmt.Errorf("ELF layout or entry address does not fit ELF32")
	}
	if fileSize > MaxELFSize {
		return nil, fmt.Errorf("ELF image exceeds %d-byte in-memory limit", MaxELFSize)
	}
	b := elfBytes{data: make([]byte, 0, int(fileSize)), is64: is64}
	class := byte(1)
	if is64 {
		class = 2
	}
	b.data = append(b.data, 0x7f, 'E', 'L', 'F', class, 1, 1, 0)
	b.pad(16)
	b.u16(2)
	b.u16(0xf3)
	b.u32(1)
	b.word(start)
	b.word(ehsize)
	b.word(shoff)
	b.u32(0)
	for _, v := range []uint16{uint16(ehsize), uint16(phsize), uint16(phnum), uint16(shsize), uint16(len(sections) + 2), uint16(len(sections) + 1)} {
		b.u16(v)
	}
	for i, s := range sections {
		if s.Flags&SHFAlloc == 0 {
			continue
		}
		flags := SectionFlagsToProgramFlags(s.Flags)
		b.u32(1)
		if is64 {
			b.u32(flags)
		}
		for _, v := range []uint64{offsets[i], s.Addr, s.Addr, uint64(len(s.Bytes)), uint64(len(s.Bytes))} {
			b.word(v)
		}
		if !is64 {
			b.u32(flags)
		}
		b.word(s.Align)
	}
	for i, s := range sections {
		b.pad(offsets[i])
		b.data = append(b.data, s.Bytes...)
	}
	b.data = append(b.data, shstrtab...)
	b.pad(shoff + shsize)
	all := append(append([]ELFSection(nil), sections...), ELFSection{Name: ".shstrtab", Bytes: shstrtab, Align: 1, Type: 3})
	for i, s := range all {
		b.u32(nameOffsets[i])
		b.u32(s.Type)
		for _, v := range []uint64{s.Flags, s.Addr, offsets[i], uint64(len(s.Bytes))} {
			b.word(v)
		}
		b.u32(s.Link)
		b.u32(s.Info)
		b.word(s.Align)
		b.word(s.Entsize)
	}
	if b.err != nil {
		return nil, b.err
	}
	return b.data, nil
}
