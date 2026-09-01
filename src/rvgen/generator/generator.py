from rvgen.instgen.helper import ISAInstrClass
from dataclasses import dataclass
from pathlib import Path
import jinja2

import rvgen.generator.config as config
from rvgen.generator.coregenerator import CoreGenerator
from rvgen.utils.elfbuilder import ElfSection, ElfBuilder


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
        self.params =generatorParams
        self.cores = [CoreGenerator() for _ in range(self.params.num_cores)]
        self._update_inst_weights()

    def _update_inst_weights(self):
        if len(self.cores) == 1:
            config.inst_weights[ISAInstrClass.SEND_IPI] = 0.0
            config.inst_weights[ISAInstrClass.CLEAR_INTERRUPT] = 0.0
            config.inst_weights[ISAInstrClass.WFI_TRAP] = 0.0
            config.inst_weights[ISAInstrClass.WFI_NOTRAP] = 0.0

    def generate(self):
        for i, core in enumerate(self.cores):
            core.generate()

    def gen_assembly(self, output_path: str | Path):
        env = jinja2.Environment(loader=jinja2.FileSystemLoader("templates"))
        template = env.get_template("riscv64.S.j2")
        with open(output_path, "w") as f:
            f.write(template.render(basic_blocks=self.cores[0].bbs))
        
    

    def gen_elf(self, output_path: str | Path):
        if len(self.cores) != 1:
            raise Exception("Only one core is supported for now.")
        bytecode = self.cores[0].get_bytecode()
        textSection = ElfSection(
            name=".text",
            inbytes=bytecode,
            addr=0x0,
            flags=0x6,  # SHF_ALLOC | SHF_EXECINSTR
        )
        elfbuilder = ElfBuilder()
        elf_bytes = elfbuilder.build([textSection], is_64bit=self.params.is_64bit, start_addr=self.params.start_addr)
        elfbuilder.save(elf_bytes=elf_bytes, destination_path=output_path)
