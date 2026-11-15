# Feature benchmark

The API guide compares eight configurations across full and sparse reads:
HDF5 versus raw, signed i8 versus i16, direct complex32 versus general vector
reads, gzip compression, and default versus capped HDF5 chunk caches.

`feature-matrix.h5` contains all five timed repeats, medians, extrema, sample
counts, fixture sizes and exact-correctness flags for each of the 16 cases.
Its metadata records the workload, CPU, filesystem, toolchain and source identity.
The fixtures are deterministic compressible I/Q staircases, not private RF.
They are temporary and are deleted after the benchmark. Reports are retained.

To reproduce on another system:

```sh
mkdir -p reports
cargo run --release --example bench_features -- reports/feature-matrix.h5
```

Choose a new output path for each run. HDF5 is a native build dependency. The
benchmark does one untimed warm-up and five timed repeats, retaining the reader
and warming the OS cache. Timers include returned-vector allocation and all
reads in each selection; validation happens outside the timer. The measured
operations are reads, not fixture generation, durable writing or acquisition.

The full pattern reads 2,097,152 samples in one request. The sparse pattern
reads 32 windows of 4096 samples, spaced 65,536 apart. General vector reads
select one subchannel and return complex32 directly; the benchmark does not add
a complex64 convenience conversion. Compare configurations within a pattern,
not full-read time against sparse-read time.

For the measured machine, default gzip-i8 full reads took 51.49 ms versus
18.27 ms for raw. Uncompressed contiguous HDF5 took 2.88 ms, so raw is not
universally faster. The direct HDF5 API also avoids much of the general vector
API's allocation and promotion overhead. Repeated sparse compressed reads
benefit from a cache large enough to retain their chunks.

The public guide's table is generated from the checked report:

```sh
conda run -n base python docs/benchmarks/update_table.py
make -C docs
```

Replacing the report requires updating its machine/source metadata and the
interpretation in the guide. A different benchmark is a new observation, not
an automatic replacement for measurements made on another system.
