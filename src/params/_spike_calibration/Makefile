RISCV_PREFIX = riscv64-unknown-elf

RV_ARCH_FLAGS64 ?= -march=rv64g -mabi=lp64
RISCV_GCC_OPTS ?= -DPREALLOCATE=1 -mcmodel=medany -static -std=gnu99 -O2 -ffast-math -fno-common -fno-builtin-printf -fno-tree-loop-distribute-patterns -gdwarf-2
LDFLAGS = -Tlink.ld -static -nostdlib -nostartfiles -lm -lgcc
CC = $(RISCV_PREFIX)-gcc

all: main.S
	$(CC) $(RV_ARCH_FLAGS64) $(RISCV_GCC_OPTS) $^ -o calibrate.elf $(LDFLAGS)

.PHONY: clean
clean:
	rm -rf build