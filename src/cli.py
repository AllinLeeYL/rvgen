import argparse
from proggen import TestCaseGenerator
from params import TestParams
from genelf import genelf
from preisasim.spikeresolution import spike_resolution
import random

def parse_bool(val):
    return str(val).lower() in ("1", "true", "yes")

def parse_argument():
    parser = argparse.ArgumentParser(description='RISC-V instruction generator')
    parser.add_argument('--size', type=int, default=256,
                        help='Program size / instruction number')
    parser.add_argument("--memsize", type=int, default=4096,
                        help="Size of the memory in bytes")
    parser.add_argument("--num-cores", type=int, default=1,
                        help="Number of cores allocated to fuzzing")
    parser.add_argument("--num-bbs", type=int, default=12,
                        help="Maximium number of basic blocks")
    parser.add_argument("--seed", type=int, default=0, metavar="SEED",
                        help="Fuzzes a single elf, given its random seed")
    parser.add_argument("--authorize-privileges", type=parse_bool, default=True,
                        help="Authorize privileges (default: True)")
    args = parser.parse_args()
    return args

def main():
    args = parse_argument()
    params = TestParams(
        design_name="test",
        memsize=args.memsize,
        randseed=args.seed,
        nmax_bbs=args.num_bbs,
        authorize_privileges=args.authorize_privileges,
        local_random=random.Random(args.seed),
    )
    generator = TestCaseGenerator(params)
    success = generator.gen_program(params)
    generator.expected_regvals = spike_resolution(generator, params)
    genelf.gen_elf_from_bbs(generator, True, "test", params, False)
    # for core in generator.corestates.values():
    #     for bb in core.basic_blocks:
    #         for inst in bb:
    #             print(inst)

if __name__ == "__main__":
    main()
