from dataclasses import dataclass
from rvgen.generator.basicblockgenerator import BasicBlockGeneratorParams, BasicBlockGenerator

@dataclass
class CoreGeneratorParams:
    num_bbs: int = 1


class CoreGenerator:
    def __init__(self, coreGeneratorParams: CoreGeneratorParams = CoreGeneratorParams()) -> None:
        self.params = coreGeneratorParams
        self.bbs = [BasicBlockGenerator(
            BasicBlockGeneratorParams(
                label=f"bb_{i}",
            )
        ) for i, _ in enumerate(range(self.params.num_bbs))]

    def generate(self):
        for i, bb in enumerate(self.bbs):
            bb.generate()

    
    def get_bytecode(self) -> bytes:
        return b""

    
    def get_section_addr(self) -> int:
        return 0