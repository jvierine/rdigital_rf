# Read benchmarks

Eight configurations, two access patterns: HDF5/raw, i8/i16, direct/vector APIs,
gzip, and default/capped HDF5 caches. Results are in `feature-matrix.h5`.

```sh
mkdir -p reports
cargo run --release --example bench_features -- reports/feature-matrix.h5
```

The output path must be new. Fixtures are temporary; the HDF5 report is retained.

- One channel and subchannel; 2,097,152 complex samples per fixture.
- Full selection: one request for all samples.
- Sparse selection: 32 windows of 4096 samples, spaced 65,536 samples apart.
- One warm-up, five timed repeats, same reader, warm OS cache.
- Direct API: `read_complex32`. Vector API: `read_vector`, one subchannel,
  complex32 output.
- Timers include reads and returned-vector allocation. Exact-value checks run
  outside the timer. All outputs must match.
- Signal: deterministic compressible I/Q staircase; i16 values scaled by 256.
- HDF5: contiguous uncompressed datasets, or gzip-1 with 65,536-sample chunks.
- Cache: default 64 MiB minimum, or 128 KiB per-file ceiling.

The report stores each timing, median, minimum, maximum, sample count, fixture
size and correctness flag. Hardware, filesystem and source identity are metadata.
Measurements cover cached reads, not sustained disk I/O or acquisition.

Rebuild the reference table from the checked report:

```sh
conda run -n base python docs/benchmarks/update_table.py
make -C docs
```
