# Implementation provenance

The initial Rust reader, integration tests and `read_vector` example were
extracted from `crates/guisdap-digital-rf/` in
[jvierine/ruisdap](https://github.com/jvierine/ruisdap) at commit
`7495937365d14b43f8ffc19e31cb965de8a0267b`.
The reader was introduced there in commit
`1f62eea9719e46696f87d366fa3e3d5f07f1524b`.

The extraction renames the Rust crate, removes workspace dependencies and
application-specific wording, checks extreme rational timestamp multiplication,
and adds standalone packaging, documentation, CI and synthetic upstream parity
tests. The running RUISDAP campaign and its frozen processing binaries are not
changed by this extraction.

The format and Python behavior reference is
[MITHaystack/digital_rf](https://github.com/MITHaystack/digital_rf), with
`digital_rf==2.6.14` used for reproducible parity tests. Upstream is independently
licensed under BSD-3-Clause. No upstream implementation files, private RF data,
campaign products or legacy analysis sources are vendored here.
