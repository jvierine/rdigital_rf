# Read benchmarks

Ten configurations, two access patterns: HDF5/raw, i8/i16, direct/vector APIs,
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
  complex32 output. Native-i8 cases use `read_vector_as::<Iq<i8>>` and return
  two signed bytes per sample.
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

## Payload control

For uncompressed HDF5, the dataset's physical payload offset is obtained before
timing. Both files are then read with the same OS seek/read calls into
preallocated byte buffers. The byte arrays must agree exactly. Both arrays
use the same conversion function and preallocated output buffer. Read order
alternates. This control excludes HDF5 API calls and metadata lookup.

The current report combines three fresh processes with five timed repeats each.
Individual reports are `audit-run-1.h5` through `audit-run-3.h5`.
To regenerate the aggregate and tables:

```sh
conda run -n base python docs/benchmarks/combine_runs.py
conda run -n base python docs/benchmarks/update_table.py
make -C docs
```
