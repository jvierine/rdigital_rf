# rdigital_rf

Digital RF in Rust. Reads Digital RF 2.x HDF5 archives and an optional raw binary
container through the same API. Provides a raw writer for local archives and
ring buffers. HDF5 is the default format.

## Install

Requires Rust and the HDF5 development library (`libhdf5-dev pkg-config` on
Debian/Ubuntu; `hdf5 pkg-config` with Homebrew).

```toml
[dependencies]
rdigital_rf = { git = "https://github.com/jvierine/rdigital_rf" }
```

## API

- [API reference (PDF)](docs/rdigital_rf-api.pdf)
- [LaTeX source](docs/rdigital_rf-api.tex); build with `make -C docs`.
- Rust reference: `cargo doc --no-deps --open`.
- [Runnable examples](https://github.com/jvierine/rdigital_rf/tree/main/examples).

`Reader::open(roots)` accepts recording roots or channel directories.
`get_bounds(channel)` returns inclusive first/last sample indices.

- `read_vector_as::<Iq<i8>>(start, length, channel, subchannel)`: complex i8 output.
- `read_vector_as::<i8>(...)`: real i8 output.
- `read_vector_as::<C64>(...)`: complex64 output, two float32 components.
- `read_vector_as::<C128>(...)`: complex128 output, two float64 components.
- `read_as::<T>(channel, start..end, subchannel)`: typed blocks, gaps and sources.

`TypedVector<T>` contains `Vec<T>` and its shape. Integer reads require matching
stored component types. Float conversion goes directly to the requested precision;
there is no float64 staging. Native contiguous raw reads on little-endian hosts fill the output
buffer directly, including all-subchannel selections. Other raw reads allocate only the selected byte span.

`read_vector` retains upstream floating-point promotion for compatibility.
`read_complex32` returns `C64`; `read` returns `Read<C128>`. Bounds and gap
semantics are shared by all paths. Indices are absolute `u64`; values are not
normalized. Cache limits apply separately from result/conversion storage.

`raw::RawWriter` writes interleaved little-endian i8/i16/f32/f64 samples with
HDF5 channel properties and index sidecars. It publishes immutable chunks;
`ring_files` limits finalized chunks. Readers detect the container automatically.
The raw extension is not supported by the upstream Python DigitalRF reader.
Ordinary HDF5 writing, Digital Metadata and remote transports are not implemented.

## Benchmarks

[Method and reproduction](docs/benchmarks/README.md) ·
[HDF5 measurements](docs/benchmarks/feature-matrix.h5)

Complex-i8, warm-cache medians over 15 repeats in three processes, milliseconds:

| Configuration / output | Full | Sparse |
|---|---:|---:|
| Uncompressed HDF5, direct C64 | 2.60 | 0.69 |
| Uncompressed HDF5, vector C64 | 2.53 | 0.71 |
| Gzip HDF5, default cache | 52.25 | 3.86 |
| Gzip HDF5, 128 KiB cache | 58.23 | 9.56 |
| Raw, direct C64 | 2.03 | 0.13 |
| Uncompressed HDF5, native i8 | 2.26 | 0.67 |
| Raw, native i8 | 0.24 | 0.04 |

Full: 2,097,152 samples. Sparse: 32 windows of 4096 samples. Hardware, cache and
layout settings, i16 results and API signatures are in the reference. A separate
control uses identical OS reads and sample conversion for both payloads. API
timings include each implementation's allocation and conversion costs.

## Validation

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo test --doc
```

[CONTRIBUTING.md](CONTRIBUTING.md) · [PROVENANCE.md](PROVENANCE.md) · BSD-3-Clause.
Independent implementation of a subset of [Digital RF](https://github.com/MITHaystack/digital_rf).
