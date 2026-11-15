# Raw container initial check

2026-11-15 UTC, DOB receiver, while ordinary acquisition and DSP continued.
The bounded `bench_raw` example compared exactly 40,000,000 archived complex-i8
samples through the same `Reader::read_complex32` API. Exact sample equality
passed, including the source's absolute sample offset.

| Operation | Seconds |
|---|---:|
| HDF5 read and widening | 1.095105 |
| Raw payload write and metadata commit | 0.062182 |
| Raw read and widening | 0.416019 |

The raw read was 2.63 times faster in this one receiver-local comparison.
This is not a sustained recorder benchmark or an end-to-end decoder result.
No live recorder has been switched to this backend. HDF5 is still the default.
The benchmark artifact stays on DOB under recorder-health/dob50-probe.

Validation: 21 integration tests passed (7 raw-specific and 14 existing/parity),
plus the documentation example. Coverage includes exact HDF5/raw agreement,
multiple subchannels, all four supported component types, explicit gaps,
cadence/hour traversal, integer timestamps, truncated payload rejection,
partial-chunk invisibility, duplicate-writer rejection and ring cache eviction.
