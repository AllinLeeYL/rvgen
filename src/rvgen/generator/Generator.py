from rvgen.instgen.helper import ISAInstrClass
from dataclasses import dataclass
from pathlib import Path
import jinja2

import rvgen.generator.config as config
from rvgen.generator.CoreGenerator import CoreGenerator


@dataclass
class GeneratorParams:
    size: int = 256
    memsize: int = 4096
    num_cores: int = 1
    num_bbs: int = 12
    seed: int = 0
    authorize_privileges: bool = True


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

    def write_assembly(self, output_path: str | Path):
        env = jinja2.Environment(loader=jinja2.FileSystemLoader("templates"))
        template = env.get_template("riscv64.S.j2")
        with open(output_path, "w") as f:
            f.write(template.render(basic_blocks=self.cores[0].bbs))
        

    def write_elf(self, output_path: str | Path):
        pass
