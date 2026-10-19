# Contributing and possible upstream inclusion

This repository is intended to make the Rust reader easy to evaluate and,
if its maintainers find it useful, integrate into
[DigitalRF](https://github.com/MITHaystack/digital_rf). Inclusion has not been
requested or agreed. Follow the upstream project's
[contribution guidelines](https://github.com/MITHaystack/digital_rf/blob/master/CONTRIBUTING.rst)
before proposing an integration there.

Keep changes independent of radar-specific pipelines. Work directly with the
Digital RF format; do not introduce RF staging, combined archive directories or
another persistent RF representation. Keep gap semantics explicit.

Before submitting a change:

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo test --doc
```

Changes to I/O semantics also need upstream comparisons with synthetic fixtures.
Use `conda run -n base python` for local Python commands. Store numerical reports
in HDF5, and keep private RF and metadata out of Git.

The initial upstream review should cover:

- Ownership of the crate, naming and preferred placement in the DigitalRF tree.
- Precise `read_vector` shape, promotion and error behavior across supported
  upstream versions and all relevant storage dtypes.
- Duplicate/overlapping root policy, malformed index handling and cache budget
  defaults.
- HDF5 and Rust version support, platform coverage and reproducible performance
  comparisons on representative compressed archives.
- APIs still missing: Digital Metadata, remote transports, writer support and
  broader reader conveniences, if those are in scope for upstream.

Do not describe this implementation as fully compatible, upstream-approved or
faster than Python without corresponding evidence. A cache-reuse test verifies
cache reuse; it is not a throughput benchmark.
