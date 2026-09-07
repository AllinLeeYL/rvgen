from rvgen.instgen.helper import ISAInstrClass
from dataclasses import dataclass
from pathlib import Path
from random import Random
import jinja2

import rvgen.generator.config as config
from rvgen.generator.coregenerator import CoreGenerator, CoreGeneratorParams
from rvgen.utils.elfbuilder import ElfBuilder
from rvgen.generator.runtime import executable


@dataclass
class GeneratorParams:
    size: int = 256
    memsize: int = 4096
    num_cores: int = 1
    num_bbs: int = 12
    seed: int = 0
    authorize_privileges: bool = True
    is_64bit: bool = True
    start_addr: int = 0x80000000


class Generator:
    def __init__(self, generatorParams: GeneratorParams):
        self.params = generatorParams
        if self.params.num_cores <= 0 or self.params.num_bbs <= 0 or self.params.size < 0:
            raise ValueError("core/block counts must be positive and size must be nonnegative")
        self.inst_weights = config.inst_weights.copy()
        self.cores = [CoreGenerator(CoreGeneratorParams(num_bbs=self.params.num_bbs, 
                        num_insts=self.params.size // self.params.num_cores,
                        )) for _ in range(self.params.num_cores)]
        self._update_inst_weights()

    def _update_inst_weights(self):
        if len(self.cores) == 1:
            self.inst_weights[ISAInstrClass.SEND_IPI] = 0.0
            self.inst_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0.0
            self.inst_weights[ISAInstrClass.WFI_TRAP] = 0.0
            self.inst_weights[ISAInstrClass.WFI_NOTRAP] = 0.0

    def generate(self):
        prng = Random(self.params.seed)
        for core in self.cores:
            core.generate(prng, self.inst_weights)

    def gen_assembly(self, output_path: str | Path):
        env = jinja2.Environment(loader=jinja2.FileSystemLoader("templates"))
        template = env.get_template("riscv64.S.j2")
        with open(output_path, "w") as f:
            f.write(template.render(basic_blocks=self.cores[0].bbs))
    

    def gen_elf(self, output_path: str | Path):
        if len(self.cores) != 1:
            raise ValueError("only one core is supported for ELF output")
        elfbuilder = ElfBuilder()
        elf_bytes = executable(self.cores[0].get_bytecode(), self.params.is_64bit, self.params.start_addr)
        elfbuilder.save(elf_bytes=elf_bytes, destination_path=output_path)
