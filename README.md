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
- [Runnable examples](examples).

`Reader::open(roots)` accepts recording roots or channel directories.
`get_bounds(channel)` returns inclusive first/last sample indices.
`read_complex32(start, length, channel, subchannel)` returns a contiguous complex32
vector. `read_vector` selects one or all subchannels and returns promoted sample
values and shape. Both return `MissingSamples` for gaps.
`read(channel, start..end, subchannel)` returns available blocks, gaps and sources.
Indices are absolute `u64`; rates are rational. Values are not normalized.

`raw::RawWriter` writes interleaved little-endian i8/i16/f32/f64 samples with
HDF5 channel properties and index sidecars. It publishes immutable chunks;
`ring_files` limits finalized chunks. Readers detect the container automatically.
The raw extension is not supported by the upstream Python DigitalRF reader.
Ordinary HDF5 writing, Digital Metadata and remote transports are not implemented.

## Benchmarks

[Method and reproduction](docs/benchmarks/README.md) ·
[HDF5 measurements](docs/benchmarks/feature-matrix.h5)

Complex-i8, warm-cache medians over 15 repeats in three processes, milliseconds:

| Configuration | Full | Sparse |
|---|---:|---:|
| Uncompressed HDF5, direct API | 2.68 | 0.59 |
| Uncompressed HDF5, vector API | 25.19 | 0.85 |
| Gzip HDF5, default cache | 51.77 | 3.66 |
| Gzip HDF5, 128 KiB cache | 57.32 | 9.53 |
| Raw, direct API | 2.07 | 0.13 |

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
