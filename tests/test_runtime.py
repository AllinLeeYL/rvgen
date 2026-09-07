from io import BytesIO
import os
from pathlib import Path
import shutil
import subprocess

from elftools.elf.elffile import ELFFile
import pytest

from rvgen.generator.generator import Generator, GeneratorParams
from rvgen.generator.runtime import executable
from rvgen.riscv import rv32i_addi

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize("bits", [32, 64])
def test_runtime_layout_and_rust_fixture(bits):
    image = executable(b"", bits == 64)
    assert image == (ROOT / f"rvgen/tests/fixtures/runtime{bits}.bin").read_bytes()
    elf = ELFFile(BytesIO(image))
    text = elf.get_section_by_name(".text")
    assert text["sh_addr"] == elf.header["e_entry"] == 0x80000000
    assert text.data()[:4] != bytes(4)
    symtab = elf.get_section_by_name(".symtab")
    for name in ["tohost", "fromhost"]:
        section = elf.get_section_by_name("." + name)
        symbol = symtab.get_symbol_by_name(name)[0]
        assert symbol["st_info"]["bind"] == "STB_GLOBAL"
        assert symbol["st_value"] == section["sh_addr"]
        assert symbol["st_size"] == 8
        assert section["sh_addr"] % 64 == 0
        assert section["sh_flags"] == 3
        assert section.data() == bytes(8)


@pytest.mark.parametrize("bits", [32, 64])
@pytest.mark.parametrize("body,expected", [
    (b"", 0), (b"\x13\x00\x00\x00" * 2048, 0),
    (b"\x01\x00", 0), (b"\xff\xff\xff\xff", 1),
])
def test_spike_exits_on_success_and_trap(tmp_path, bits, body, expected):
    spike = os.environ.get("SPIKE") or shutil.which("spike")
    if spike is None:
        pytest.skip("Spike is unavailable")
    path = tmp_path / "test.elf"
    path.write_bytes(executable(body, bits == 64, 0x80000100))
    result = subprocess.run([spike, "-m64", f"--isa=rv{bits}gc", str(path)],
                            capture_output=True, timeout=5)
    assert result.returncode == expected, result.stderr.decode()
    assert b"symbols not in ELF" not in result.stderr


def test_generator_body_serialization_seed_and_weight_isolation(tmp_path):
    first = Generator(GeneratorParams(seed=42, size=20, num_bbs=2))
    second = Generator(GeneratorParams(seed=42, size=20, num_bbs=2))
    first.generate()
    second.generate()
    assert first.cores[0].bbs[0].selected_classes == second.cores[0].bbs[0].selected_classes
    first.cores[0].bbs[0].insts = [rv32i_addi(10, 0, 42)]
    assert first.cores[0].get_bytecode() == b"\x13\x05\xa0\x02"
    first.gen_elf(tmp_path / "test.elf")
    elf = ELFFile(BytesIO((tmp_path / "test.elf").read_bytes()))
    assert elf.get_section_by_name(".text").data()[12:16] == first.cores[0].get_bytecode()
    multi = Generator(GeneratorParams(num_cores=2))
    assert first.inst_weights != multi.inst_weights


def test_runtime_rejects_bad_addresses_and_incomplete_instructions():
    for body, bits, address in [(b"x", True, 0x80000000), (b"", True, 0x80000002),
                                (b"", False, 0xfffffff0), (b"", True, (1 << 64) - 4)]:
        with pytest.raises(ValueError):
            executable(body, bits, address)
