from dataclasses import dataclass
from random import Random
from itertools import accumulate
import math

import rvgen.generator.config as config
from rvgen.instgen.helper import ISAInstrClass

@dataclass
class BasicBlockGeneratorParams:
    num_insts: int = 8
    label: str = ""


class BasicBlockGenerator:
    def __init__(self, basicBlockGeneratorParams: BasicBlockGeneratorParams = BasicBlockGeneratorParams()) -> None:
        self.params = basicBlockGeneratorParams
        self.insts = [] # the final instructions to output
        self.selected_classes = []

    def __getitem__(self, index: int):
        return self.insts[index]

    def __iter__(self):
        return iter(self.insts)

    def __len__(self):
        return len(self.insts)

    def generate(self, prng: Random | None = None, weights=None):
        """
        Sample instruction classes; random instruction construction is still a scaffold.
        """
        prng = prng if prng is not None else Random()
        weights = config.inst_weights if weights is None else weights
        values = list(weights.values())
        if (not values or any(not math.isfinite(w) or w < 0 for w in values)
                or not math.isfinite(sum(values)) or sum(values) <= 0):
            raise ValueError("instruction weights must be finite, nonnegative, and have a positive total")
        self.selected_classes = prng.choices(list(weights), cum_weights=list(accumulate(values)),
                                            k=self.params.num_insts)

    def _choose_inst(self, prng: Random | None = None) -> ISAInstrClass:
        """
        Choose an instruction class randomly.
        """
        prng = prng if prng is not None else Random()
        instCls = prng.choices(
            population=list(config.inst_weights.keys()),
            weights=list(config.inst_weights.values())
        )[0]
        assert config.inst_weights[instCls] != 0
        return instCls
