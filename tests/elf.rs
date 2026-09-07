use object::{Object, ObjectSection, ObjectSegment};
use rvgen::elf::{ElfBuilder, ElfSection, SHF_ALLOC, SHF_WRITE};

fn sections() -> Vec<ElfSection> {
    let mut text = ElfSection::new(".text", vec![0x13, 0, 0, 0, 0x73, 0, 0x10, 0]);
    text.addr = 0x8000_0000;
    let mut data = ElfSection::new(".data", (0..17).collect::<Vec<u8>>());
    data.addr = 0x8000_1000;
    data.flags = SHF_ALLOC | SHF_WRITE;
    data.align = 16;
    let mut comment = ElfSection::new(".comment", b"fixture\0".to_vec());
    comment.flags = 0;
    comment.align = 1;
    vec![text, data, comment]
}

#[test]
fn independent_reader_checks_sections_segments_and_entry() {
    for is_64bit in [false, true] {
        let sections = sections();
        let bytes = ElfBuilder.build(&sections, is_64bit, 0x8000_0000).unwrap();
        let file = object::File::parse(bytes.as_slice()).unwrap();
        assert_eq!(file.is_64(), is_64bit);
        assert!(file.is_little_endian());
        assert_eq!(
            file.architecture(),
            if is_64bit {
                object::Architecture::Riscv64
            } else {
                object::Architecture::Riscv32
            }
        );
        assert_eq!(file.kind(), object::ObjectKind::Executable);
        assert_eq!(file.entry(), 0x8000_0000);
        for expected in &sections {
            let section = file.section_by_name(&expected.name).unwrap();
            assert_eq!(section.address(), expected.addr);
            assert_eq!(section.data().unwrap(), expected.inbytes);
            assert_eq!(section.align(), expected.align);
            assert_eq!(section.file_range().unwrap().0 % expected.align, 0);
        }
        let segments: Vec<_> = file.segments().collect();
        assert_eq!(segments.len(), 2);
        for (segment, expected) in segments.iter().zip(&sections) {
            assert_eq!(segment.address(), expected.addr);
            assert_eq!(segment.data().unwrap(), expected.inbytes);
            assert_eq!(
                segment.flags(),
                object::SegmentFlags::Elf {
                    p_flags: ElfBuilder::sh_flags_to_p_flags(expected.flags),
                }
            );
        }
    }
}

#[test]
fn rejects_invalid_or_unrepresentable_layouts() {
    let mut section = ElfSection::new(".text", vec![0; 4]);
    section.align = 3;
    assert!(ElfBuilder.build(&[section.clone()], true, 0).is_err());
    section.align = 4;
    section.addr = u64::MAX;
    assert!(ElfBuilder.build(&[section.clone()], true, 0).is_err());
    section.addr = 0x1_0000_0000;
    assert!(ElfBuilder.build(&[section.clone()], false, 0).is_err());
    section.addr = 0;
    assert!(ElfBuilder
        .build(&[section.clone()], false, 0x1_0000_0000)
        .is_err());
    assert!(ElfBuilder
        .build(&[section.clone(), section.clone()], true, 0)
        .is_err());
    for name in ["", ".shstrtab", "bad\0name", "é"] {
        section.name = name.into();
        assert!(ElfBuilder.build(&[section.clone()], true, 0).is_err());
    }
}

#[test]
fn supports_empty_section_list_and_64bit_addresses() {
    let bytes = ElfBuilder.build(&[], true, 0).unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    assert_eq!(file.segments().count(), 0);
    assert!(file.section_by_name(".shstrtab").is_some());
    let mut section = ElfSection::new(".text", vec![0x13, 0, 0, 0]);
    section.addr = 0xffff_ffff_8000_0000;
    let bytes = ElfBuilder
        .build(&[section], true, 0xffff_ffff_8000_0000)
        .unwrap();
    let file = object::File::parse(bytes.as_slice()).unwrap();
    assert_eq!(file.entry(), 0xffff_ffff_8000_0000);
}
