import os
from pathlib import Path
import subprocess
import sys

import pytest

from rvgen.cli import main, parse_argument, output_path
from rvgen.generator.generator import Generator

ROOT = Path(__file__).resolve().parents[1]


def test_batch_config_overrides_and_seed_sequence(tmp_path, monkeypatch, capsys):
    config = tmp_path / "config.toml"
    config.write_text("count = 2\nseed = 10\nsize = 0\nnum_bbs = 3\nmemsize = 8192\n"
                      "authorize_privileges = false\nout = 'case.elf'\nout_dir = 'wrong'")
    params = []
    original = Generator.generate

    def record(self):
        params.append(self.params)
        original(self)

    monkeypatch.setattr(Generator, "generate", record)
    directory = tmp_path / "nested/batch"
    assert main(["--config", str(config), "--count", "3", "--out-dir", str(directory)]) == 0
    assert sorted(p.name for p in directory.iterdir()) == [f"case_{i:06}.elf" for i in range(3)]
    assert [p.seed for p in params] == [10, 11, 12]
    assert all(p.num_bbs == 3 and p.size == 0 and p.memsize == 8192 and not p.authorize_privileges for p in params)
    assert "Generated 3 ELF(s)" in capsys.readouterr().err


def test_single_file_and_batch_naming(tmp_path):
    args = parse_argument(["--count", "3", "-o", "prefix/case"])
    assert output_path(args, 2) == Path("prefix/case_000002.elf")
    args = parse_argument(["--out-dir", "batch", "-o", "case.elf"])
    assert output_path(args, 0) == Path("batch/case.elf")
    out = tmp_path / "chosen.elf"
    assert main(["--size", "0", "-o", str(out)]) == 0
    assert out.read_bytes() == (ROOT / "rvgen/tests/fixtures/runtime64.bin").read_bytes()


@pytest.mark.parametrize("args", [
    ["--count", "0"], ["--count", "-1"], ["--num-cores", "0"], ["--size", "-1"],
    ["--num-bbs", "0"], ["--count", "2", "--seed", "9223372036854775807"],
])
def test_invalid_arguments(args):
    with pytest.raises(SystemExit):
        parse_argument(args)


def test_binary_errors_and_config_types(tmp_path):
    env = dict(os.environ, PYTHONPATH=str(ROOT / "src"))
    path = tmp_path / "output.elf"
    path.write_bytes(b"keep existing file")
    for args in [["--num-cores", "2"], ["--config", "missing.toml"], ["--out", "missing/out.elf"]]:
        result = subprocess.run([sys.executable, "-m", "rvgen.cli", *args], cwd=tmp_path,
                                env=env, capture_output=True)
        assert result.returncode != 0
        assert path.read_bytes() == b"keep existing file"
    for source in ["count = '3'", "size = true", "out_dir = 5", "authorize_privileges = 'false'", "unknown = 1"]:
        config = tmp_path / "bad.toml"
        config.write_text(source)
        with pytest.raises(SystemExit):
            parse_argument(["--config", str(config)])
