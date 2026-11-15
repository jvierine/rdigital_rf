# rdigital_rf

Native Rust, read-only access to [Digital RF](https://github.com/MITHaystack/digital_rf)
2.x archives. Read original HDF5 files across multiple recording directories,
without combining directories or converting RF into another format.

This is an independent implementation extracted from the RUISDAP processing
pipeline. The aim is to make a small, testable Rust reader available for eventual
review and possible inclusion in upstream DigitalRF. It is not an official
DigitalRF component or a complete replacement for its Python API.

## Use

Install Rust and the native HDF5 development library. On Ubuntu/Debian:

```sh
sudo apt-get install libhdf5-dev pkg-config
```

On macOS with Homebrew:

```sh
brew install hdf5 pkg-config
```

The crate has not been published to crates.io. Add it from Git:

```toml
[dependencies]
rdigital_rf = { git = "https://github.com/jvierine/rdigital_rf" }
```

```rust,no_run
use rdigital_rf::{CacheOptions, Reader};

fn main() -> anyhow::Result<()> {
    let mut reader = Reader::open(["/archive/run-a", "/archive/run-b"])?;
    reader.set_cache_options(CacheOptions {
        minimum_bytes: 64 * 1024 * 1024,
        max_open_files: 8,
    })?;

    // Absolute sample index, sample count, channel, optional subchannel.
    let vector = reader.read_vector(17_125_992_100_000, 1024, "rx", Some(0))?;
    println!("shape: {:?}", vector.shape);
    let samples = vector.into_complex64();
    println!("{} complex samples", samples.len());
    Ok(())
}
```

The application above also needs `anyhow = "1"` for its example return type.

## Implemented behavior

- Multiple top-level channel directories, recording directories containing
  `rf_data/`, or individual channel directories. Duplicate directory arguments
  are deduplicated.
- Absolute `u64` sample indices and rational sample rates. Timestamp arithmetic
  uses checked integers, avoiding a round trip through floating point.
- `read_vector(start, length, channel, subchannel)` with strict continuity,
  row-major subchannels and floating/complex promotion modeled on upstream
  Python `read_vector`.
- `read(channel, start..end, subchannel)` returns measured blocks, exact missing
  intervals and source spans. Its range is half-open. `bounds(channel)` returns
  inclusive first/last stored sample indices.
- `rf_data_index` is authoritative. Missing files and indexed gaps never become
  zeros. Strict reads return a downcastable `MissingSamples` error, and later
  valid reads remain possible.
- Bounded open-file LRU and HDF5 decompression caches: each file cache is at
  least 64 MiB by default and at least the full uncompressed logical dataset
  size. This avoids repeatedly decompressing chunks during sparse reads. Cache
  bytes are a minimum per open file, **not a total memory cap**; lower the file
  count for memory-constrained applications. No persistent RF cache is written.

`Vector.samples` is one of `Real32`, `Real64`, `Complex32` or `Complex64`.
This follows upstream floating-point promotion; integer storage is not returned
as an integer vector. `None` selects all subchannels, `Some(i)` selects one.
As in the tested upstream version, a one-sample request selecting multiple
subchannels is rejected; selecting one subchannel is supported.

## Compatibility boundaries

Only local filesystems (including mounted NFS) and Digital RF 2.x files with
rational-rate properties and the Unix epoch are supported. HTTP/FTP transports,
Digital Metadata, writers, directory watching and the remaining Python reader
API are not implemented. New files following the already-discovered channel
layout can be found by subsequent reads, but there is no live-writer consistency
or SWMR guarantee. Malformed HDF5 and unsupported layouts are errors.

Same-name channels across roots must have compatible promoted sample format,
rate, epoch and subchannel count. Identical overlapping samples are accepted;
conflicting overlaps are rejected. Upstream overlap policy and a broader dtype
and format matrix remain review items. Do not infer full API compatibility from
the currently tested cases.

## Validation

```sh
cargo test --all-targets
cargo test --doc
cargo fmt --all -- --check
cargo build --example read_vector
conda run -n base python -m pip install -r tests/requirements.txt
conda run -n base python tests/compare_upstream.py \
  --binary target/debug/examples/read_vector --report reports/parity.h5
```

Rust tests cover compressed file/hour/root boundaries, indexed gaps,
subchannels, cache reuse, rational sample rates, conflicting overlaps and
indices above `2^53`. The parity runner creates synthetic recordings with the
official Python writer and compares exact values, shapes, promoted dtypes and
gap errors against `digital_rf==2.6.14`. Reports are HDF5. No private recordings
or campaign-derived data are distributed in this repository.

For downstream integration, API review and outstanding work, see
[CONTRIBUTING.md](CONTRIBUTING.md). The implementation's extraction and reference
history are recorded in [PROVENANCE.md](PROVENANCE.md).

## License

BSD-3-Clause. DigitalRF is a separate project; its name does not imply
endorsement of this implementation.

## Bounded streaming receiver reads

`Reader::read_complex32(start, length, channel, subchannel)` returns a contiguous
`Vec<Complex32>` and reports indexed gaps as `MissingSamples`. Native complex
int16 recordings convert directly to complex float32; other supported formats
use the general reader. Integer sample arithmetic preserves absolute indices
above `2^53`. Set `CacheOptions::max_open_files` and call
`set_cache_byte_limit(bytes)` to cap the HDF5 raw chunk cache per open file.
This ceiling changes cache reuse, not sample values or gap handling. It does
not limit the requested output vector, so streaming clients should bound their
read lengths. These APIs are used by the experimental Rust ionosonde receiver.

### Optional raw binary container

Ordinary Digital RF HDF5 archives remain the default. The Rust reader also
supports the opt-in `rdigital_rf` raw-container extension (version 1). This is
not a format supported by the upstream Python DigitalRF reader.

`Reader::open` detects either container automatically. `get_bounds`/`bounds`,
`read`, `read_vector` and `read_complex32` keep their sample indices, promotion,
subchannel and gap behavior. Mixed recording roots are allowed when their
channel properties agree. No missing samples are filled with zeros.

To write raw chunks, explicitly construct `raw::RawWriter` with `raw::Options`.
The default options describe signed 8-bit complex samples, one subchannel,
25 MS/s and one-second files. Set the rate, type and layout to match the input;
`write(start_sample, bytes)` accepts little-endian bytes in sample, subchannel,
real/imaginary order. Supported components are signed i8/i16 and IEEE f32/f64;
real recordings are supported too. Values are unscaled counts. `ring_files`
optionally limits the number of finalized chunks; `None` retains an archive.

`drf_properties.h5` describes the channel. Each finalized `rf@SECONDS.MMM.raw`
has a small `rf@SECONDS.MMM.h5` sidecar containing the format version, type,
shape and `rf_data_index`. The sidecar is the commit marker: the payload is
published first, metadata last. Readers ignore partial chunks. A chunk is
immutable after publication; `flush()` finalizes it, so subsequent writes must
use another cadence interval. Open readers may retain an evicted inode until
cache eviction, but never read reused storage under an old index.

There is one writer per channel. Version 1 requires a new channel directory;
after a process crash, inspect partial files and the writer lock before
starting a new recording directory. Dropping a writer deliberately does not
publish pending samples: call `flush()` to finalize them. `durable=true` syncs
payload, metadata and the commit directory; RAM rings normally leave it off.
Cadences must align to integer sample counts and divide the subdirectory period.
HDF5 metadata changes remain outside the high-rate sample read/write path.
