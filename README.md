# rvgen

A standalone RISC-V instruction generator that does not depend on unrelated simulation tools or remain tightly coupled to a fuzzing framework. (still under development)

## Note for function invocation flow

```mermaid
graph LR
    A["TestCaseGenerator.gen_program()"] --> B["spike_resolution()"]

    B --> C

    subgraph C["gen_elf_from_bbs()"]
        direction TB
        C1["gen_regdump_reqs()"]
        C2["asymmetric_isa_presim()"]
        C3["_feed_regdump_to_instrs()"]

        C1 --> C2
        C2 --> C3
    end
```
