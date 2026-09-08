# Local throughput comparison

Both implementations were built normally for execution: `go build` and
`cargo build --release --locked`. Measurements used Linux x86-64, Go 1.27.1,
and rustc 1.96.0. Each cell is the median of five runs, alternating which
implementation ran first. No builds or test suites ran alongside these samples.

| Workload per ELF | Files per run | Rust ELF/s | Go ELF/s | Go / Rust |
| --- | ---: | ---: | ---: | ---: |
| 256 instructions | 2,000 | 9,503 | 15,853 | 1.67× |
| 4,096 instructions | 300 | 776 | 2,155 | 2.78× |

Rates use each executable's reported generation/write duration. Output goes into
a fresh temporary directory for each run; cleanup occurs after timing. Seed starts
at 42, with the normal consecutive seeds, 12 blocks, and default generation policy.
Full observations, including process wall time, are in [benchmarks.json](benchmarks.json).

This compares the two implementations on this machine, not the languages in
isolation. Go uses its standard RNG instead of Rust's ChaCha RNG, precomputes
class membership, and reuses small operand/sampling buffers. The generated
instruction streams consequently differ. Filesystem cache and machine load affect
these results; no disk flush is requested. Small single-file invocations also
include process startup costs not reflected in the reported generation timer.

To repeat the workload from the repository root, with both toolchains installed:

```sh
cargo build --release --locked
go -C go build -o bin/rvgen ./cmd/rvgen

benchmark_dir=$(mktemp -d)
./target/release/rvgen --size 256 --num-elfs 2000 --seed 42 --out-dir "$benchmark_dir/rust"
./go/bin/rvgen --size 256 --num-elfs 2000 --seed 42 --out-dir "$benchmark_dir/go"
```

Repeat five times with fresh directories and alternating order, then take the
median ELF/s. For the larger case, use `--size 4096 --num-elfs 300`. Remove the
created temporary directories after recording the results.

For Go-only generation and in-memory ELF benchmarks, excluding filesystem writes:

```sh
cd go
go test -run '^$' -bench . -benchmem
```
