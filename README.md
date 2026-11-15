# rdigital_rf

**Digital RF in Rust:** read time-indexed radio samples from Digital RF HDF5
recordings, or use an optional raw binary container for high-rate acquisition
and ring buffers. Both containers use the same Rust reader API, with absolute
sample indices, rational sample rates and explicit gaps.

The crate reads original [Digital RF](https://github.com/MITHaystack/digital_rf)
2.x archives across recording directories without converting or combining them.
The new **raw container** stores sample bytes outside HDF5 while retaining small
HDF5 files for channel properties and sample indices. HDF5 remains the default;
raw writing is selected explicitly through `RawWriter`.

This is an independent implementation, with a reader and an experimental raw
writer. It is not an official DigitalRF component or a complete implementation
of the Python API. In particular, the upstream Python reader does **not** read
the raw extension, and this crate does not yet write ordinary Digital RF HDF5
recordings.

## Getting started

Install Rust and the HDF5 development library (`libhdf5-dev pkg-config` on
Debian/Ubuntu, or `hdf5 pkg-config` with Homebrew). The crate is not yet on
crates.io. Add it from Git:

```toml
[dependencies]
rdigital_rf = { git = "https://github.com/jvierine/rdigital_rf" }
anyhow = "1"
```

```rust,no_run
use rdigital_rf::Reader;

fn main() -> anyhow::Result<()> {
    let mut reader = Reader::open(["/archive/run-a", "/archive/run-b"])?;
    if let Some((first, last)) = reader.get_bounds("ch0")? {
        let count = (last - first).min(1023) as usize + 1;
        // Absolute start, sample count, channel, subchannel.
        let iq = reader.read_complex32(first, count, "ch0", 0)?;
        println!("{} samples, first I/Q: {:?}", iq.len(), iq[0]);
    }
    Ok(())
}
```

Bounds describe the first and last stored samples, inclusively; gaps can exist
between them. Strict reads return a `MissingSamples` error for missing data.
For gap-aware processing, use `read("ch0", start..end, 0)` and inspect its
measured blocks and missing intervals. No missing samples are filled with zeros.

## Documentation and examples

- [API guide (PDF)](docs/rdigital_rf-api.pdf): reader reference, sample model,
  error handling, cache controls, raw layout and working examples.
- [LaTeX source](docs/rdigital_rf-api.tex); rebuild with `make -C docs` using LaTeX.
- [Runnable examples](examples): bounded streaming reads, gap-aware reads and a
  raw ring-buffer round trip.
- Generate the Rust reference with `cargo doc --no-deps --open`.

## Two sample containers, one reader

| | Digital RF HDF5 | Raw extension |
|---|---|---|
| Payload | HDF5 sample datasets | Little-endian binary chunks |
| Metadata | Digital RF properties and file indices | HDF5 properties and per-chunk index sidecars |
| Read API | `Reader` | The same `Reader`, detected automatically |
| Writing in this crate | Not implemented | Explicit `raw::RawWriter` |
| Upstream Python reader | Supported format; tested subset | Not supported |
| Typical use | Existing recordings, compressed archives | Local acquisition and bounded RAM rings |

Raw supports signed i8/i16 and IEEE f32/f64 components, real or complex samples,
and multiple subchannels. Values are unscaled counts. The writer publishes
immutable chunks by renaming the binary payload first and its HDF5 commit marker
last. Incomplete chunks remain invisible. `ring_files` bounds the number of
finalized chunks; dropping a writer does not flush pending samples.

Version 1 requires a new channel directory and one writer per channel. Inspect
partial files and the lock after a crash before starting in a new recording
directory. See the guide for cadence, flushing, durability and eviction details.

## Reader behavior

Roots may contain channel directories, an `rf_data/` directory, or be channel
directories themselves. Duplicate roots are deduplicated. Compatible same-name
channels can span roots; identical overlapping samples are accepted and
conflicting overlaps are errors.

`read_vector` returns promoted real/complex floating-point vectors, selecting one
or all subchannels. `read_complex32` provides a direct path for streaming DSP,
including signed i8/i16 recordings. Absolute indices remain `u64`; sample-time
arithmetic uses checked integers. Only local filesystems and the Unix epoch are
supported. Digital Metadata, remote transports, directory watching and full
Python API compatibility are outside the current implementation.

The open-file and HDF5 caches are configurable. By default an HDF5 cache can grow
to the logical dataset size per file; its minimum is not a total memory limit.
Use `set_cache_byte_limit` and bounded reads when memory is limited. HDF5 live
writer/SWMR consistency is not guaranteed; raw readers see finalized chunks only.

## Validation

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo test --doc
cargo run --example raw_ring
```

The tests cover file/directory boundaries, explicit gaps, subchannels, signed
extremes, rational rates, indices above `2^53`, overlap policy, ring eviction,
partial publication and exact HDF5/raw sample agreement. The synthetic parity
runner in `tests/compare_upstream.py` compares HDF5 reads with the official
Python package; it writes its report as HDF5. It does not establish full upstream
compatibility.

A single bounded comparison on DOB read 40 million identical complex-i8 samples
in 1.10 s from HDF5 and 0.42 s from raw. This is a receiver-local read benchmark,
not sustained acquisition or end-to-end decoder throughput. Use
`examples/bench_raw.rs` to measure a representative workload.

## Development and license

See [CONTRIBUTING.md](CONTRIBUTING.md) for validation and integration work, and
[PROVENANCE.md](PROVENANCE.md) for the implementation's history. BSD-3-Clause.
DigitalRF is a separate project; its name does not imply endorsement.
