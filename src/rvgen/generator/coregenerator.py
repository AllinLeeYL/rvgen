from dataclasses import dataclass
import struct
from rvgen.generator.basicblockgenerator import BasicBlockGeneratorParams, BasicBlockGenerator

@dataclass
class CoreGeneratorParams:
    num_bbs: int = 1
    num_insts: int = 20


class CoreGenerator:
    def __init__(self, coreGeneratorParams: CoreGeneratorParams = CoreGeneratorParams()) -> None:
        self.params = coreGeneratorParams
        self.bbs = [BasicBlockGenerator(
            BasicBlockGeneratorParams(
                num_insts=self.params.num_insts,
                label=f"bb_{i}",
            )
        ) for i, _ in enumerate(range(self.params.num_bbs))]

    def generate(self, prng=None, weights=None):
        for i, bb in enumerate(self.bbs):
            bb.generate(prng, weights)

    
    def get_bytecode(self) -> bytes:
        return b"".join(inst if isinstance(inst, bytes) else struct.pack('<I', inst)
                        for bb in self.bbs for inst in bb.insts)

    
    def get_section_addr(self) -> int:
        return 0
