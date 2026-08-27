from dataclasses import dataclass, field
from random import Random

import rvgen.generator.config as config
from rvgen.instgen.helper import ISAInstrClass, BASE_DISTRIBUTION

@dataclass
class BasicBlockGeneratorParams:
    num_insts: int = 8
    label: str = ""


class BasicBlockGenerator:
    def __init__(self, basicBlockGeneratorParams: BasicBlockGeneratorParams = BasicBlockGeneratorParams()) -> None:
        self.params = basicBlockGeneratorParams
        self.insts = [] # the final instructions to output

    def __getitem__(self, index: int):
        return self.insts[index]

    def __iter__(self):
        return iter(self.insts)

    def __len__(self):
        return len(self.insts)

    def generate(self, prng: Random = Random()):
        """
        Generate instructions for one basic block.
        """
        for i in range(self.params.num_insts):
            instCls = self._choose_inst(prng=prng)
            print(ISAInstrClass(instCls).name)

    def _choose_inst(self, prng: Random = Random()) -> ISAInstrClass:
        """
        Choose an instruction class randomly.
        """
        instCls = prng.choices(
            population=list(config.inst_weights.keys()),
            weights=list(config.inst_weights.values())
        )[0]
        assert config.inst_weights[instCls] != 0
        return instCls
