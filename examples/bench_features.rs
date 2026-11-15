//! Reproducible warm-cache feature matrix. RF fixtures are synthetic and private
//! to a disposable directory; only an HDF5 numerical report is retained.
use anyhow::{ensure, Context, Result};
use hdf5::{
    types::{CompoundField, CompoundType, TypeDescriptor, VarLenUnicode},
    File, H5Type,
};
use rdigital_rf::{
    raw::{Component, Options, RawWriter},
    Reader, Samples,
};
use std::{path::Path, str::FromStr, time::Instant};
const N: usize = 2_097_152;
const REPEATS: usize = 5;
#[repr(C)]
#[derive(Clone, Copy)]
struct Iq8 {
    r: i8,
    i: i8,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Iq16 {
    r: i16,
    i: i16,
}
macro_rules! compound {
    ($t:ty,$v:ty,$size:expr) => {
        unsafe impl H5Type for $t {
            fn type_descriptor() -> TypeDescriptor {
                TypeDescriptor::Compound(CompoundType {
                    fields: vec![
                        CompoundField::typed::<$v>("r", 0, 0),
                        CompoundField::typed::<$v>("i", $size, 1),
                    ],
                    size: 2 * $size,
                })
            }
        }
    };
}
compound!(Iq8, i8, 1);
compound!(Iq16, i16, 2);
fn value(row: usize, bits: u32) -> (f32, f32) {
    let scale = if bits == 8 { 1. } else { 256. };
    (
        ((row / 17 % 256) as f32 - 128.) * scale,
        ((row / 13 % 256) as f32 - 128.) * scale,
    )
}
fn attr(h: &File, n: &str, v: u64) -> Result<()> {
    h.new_attr::<u64>().create(n)?.write_scalar(&v)?;
    Ok(())
}
fn text(h: &File, n: &str, v: &str) -> Result<()> {
    h.new_attr::<VarLenUnicode>()
        .create(n)?
        .write_scalar(&VarLenUnicode::from_str(v)?)?;
    Ok(())
}
fn fixture(root: &Path, bits: u32, gzip: bool) -> Result<()> {
    let ch = root.join("ch0");
    std::fs::create_dir_all(ch.join("1970-01-01T00-00-00"))?;
    let h = File::create(ch.join("drf_properties.h5"))?;
    for (n, v) in [
        ("sample_rate_numerator", N as u64),
        ("sample_rate_denominator", 1),
        ("num_subchannels", 1),
        ("is_complex", 1),
        ("file_cadence_millisecs", 1000),
        ("subdir_cadence_secs", 3600),
        ("H5Tget_class", 0),
        ("H5Tget_size", u64::from(bits / 8)),
    ] {
        attr(&h, n, v)?;
    }
    text(&h, "digital_rf_version", "2.6.0")?;
    text(&h, "epoch", "1970-01-01T00:00:00Z")?;
    let h = File::create(ch.join("1970-01-01T00-00-00/rf@0.000.h5"))?;
    if bits == 8 {
        let v: Vec<_> = (0..N)
            .map(|r| {
                let (a, b) = value(r, bits);
                Iq8 {
                    r: a as i8,
                    i: b as i8,
                }
            })
            .collect();
        let mut b = h.new_dataset::<Iq8>().shape((N, 1));
        if gzip {
            b = b.chunk((65536, 1)).deflate(1);
        }
        b.create("rf_data")?.write_raw(&v)?;
    } else {
        let v: Vec<_> = (0..N)
            .map(|r| {
                let (a, b) = value(r, bits);
                Iq16 {
                    r: a as i16,
                    i: b as i16,
                }
            })
            .collect();
        h.new_dataset::<Iq16>()
            .shape((N, 1))
            .create("rf_data")?
            .write_raw(&v)?;
    }
    h.new_dataset::<u64>()
        .shape((1, 2))
        .create("rf_data_index")?
        .write_raw(&[0, 0])?;
    Ok(())
}
fn raw_fixture(root: &Path, bits: u32) -> Result<()> {
    let mut w = RawWriter::create(
        root.join("ch0"),
        Options {
            component: if bits == 8 {
                Component::I8
            } else {
                Component::I16
            },
            sample_rate_numerator: N as u64,
            ..Options::default()
        },
    )?;
    let mut bytes = Vec::with_capacity(N * (bits / 4) as usize);
    for r in 0..N {
        let (a, b) = value(r, bits);
        for x in [a, b] {
            if bits == 8 {
                bytes.push(x as i8 as u8);
            } else {
                bytes.extend_from_slice(&(x as i16).to_le_bytes());
            }
        }
    }
    w.write(0, &bytes)?;
    w.flush()?;
    Ok(())
}
fn load(
    r: &mut Reader,
    start: usize,
    n: usize,
    vector: bool,
) -> Result<Vec<num_complex::Complex32>> {
    if vector {
        match r.read_vector(start as u64, n, "ch0", Some(0))?.samples {
            Samples::Complex32(v) => Ok(v),
            _ => anyhow::bail!("unexpected promotion"),
        }
    } else {
        r.read_complex32(start as u64, n, "ch0", 0)
    }
}
fn disk_bytes(root: &Path) -> Result<u64> {
    let mut n = 0;
    for e in std::fs::read_dir(root)? {
        let p = e?.path();
        n += if p.is_dir() {
            disk_bytes(&p)?
        } else {
            p.metadata()?.len()
        };
    }
    Ok(n)
}
fn expand(bytes: &[u8], bits: u32, out: &mut Vec<num_complex::Complex32>) {
    out.clear();
    if bits == 8 {
        out.extend(
            bytes
                .chunks_exact(2)
                .map(|b| num_complex::Complex32::new(b[0] as i8 as f32, b[1] as i8 as f32)),
        )
    } else {
        out.extend(bytes.chunks_exact(4).map(|b| {
            num_complex::Complex32::new(
                i16::from_le_bytes([b[0], b[1]]) as f32,
                i16::from_le_bytes([b[2], b[3]]) as f32,
            )
        }))
    }
}
fn audit(report: &File, root: &Path, bits: u32) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let hdf_path = root
        .join(if bits == 8 { "hdf8" } else { "hdf16" })
        .join("ch0/1970-01-01T00-00-00/rf@0.000.h5");
    let h = File::open(&hdf_path)?;
    let ds = h.dataset("rf_data")?;
    let offsets = [
        ds.offset()
            .context("contiguous payload offset unavailable")?,
        0,
    ];
    let mut files = [
        std::fs::File::open(hdf_path)?,
        std::fs::File::open(
            root.join(if bits == 8 { "raw8" } else { "raw16" })
                .join("ch0/1970-01-01T00-00-00/rf@0.000.raw"),
        )?,
    ];
    let mut payloads = [
        vec![0u8; N * (bits / 4) as usize],
        vec![0u8; N * (bits / 4) as usize],
    ];
    let mut times: [Vec<f64>; 4] = std::array::from_fn(|_| vec![]);
    let mut output = Vec::with_capacity(N);
    for rep in 0..=REPEATS {
        for side in if rep % 2 == 0 { [0, 1] } else { [1, 0] } {
            let t = Instant::now();
            files[side].seek(SeekFrom::Start(offsets[side]))?;
            files[side].read_exact(&mut payloads[side])?;
            let elapsed = t.elapsed().as_secs_f64();
            if rep > 0 {
                times[side].push(elapsed);
            }
        }
        ensure!(payloads[0] == payloads[1], "audit payloads differ");
        for side in if rep % 2 == 0 { [0, 1] } else { [1, 0] } {
            let t = Instant::now();
            expand(std::hint::black_box(&payloads[side]), bits, &mut output);
            let elapsed = t.elapsed().as_secs_f64();
            for (r, z) in output.iter().enumerate() {
                let (a, b) = value(r, bits);
                ensure!(z.re == a && z.im == b, "audit conversion differs");
            }
            if rep > 0 {
                times[side + 2].push(elapsed);
            }
        }
    }
    for (i, label) in [
        "hdf_payload",
        "raw_payload",
        "hdf_shared_conversion",
        "raw_shared_conversion",
    ]
    .iter()
    .enumerate()
    {
        let mut sorted = times[i].clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[REPEATS / 2];
        let g = report.create_group(&format!("audit_i{bits}_{label}"))?;
        g.new_dataset_builder()
            .with_data(&times[i])
            .create("seconds")?;
        g.new_dataset::<f64>()
            .create("median_seconds")?
            .write_scalar(&median)?;
        g.new_dataset::<u64>()
            .create("exact_equal")?
            .write_scalar(&1)?;
        println!(
            "audit i{bits} {label:24} median_ms={:.3} exact=true",
            median * 1000.
        );
    }
    Ok(())
}
fn native_i8_cases(report: &File, root: &Path) -> Result<()> {
    for source in ["hdf8", "raw8"] {
        for sparse in [false, true] {
            let mut r = Reader::open([root.join(source)])?;
            let selections: Vec<_> = if sparse {
                (0..32).map(|i| (i * 65536 + 1024, 4096)).collect()
            } else {
                vec![(0, N)]
            };
            let total: usize = selections.iter().map(|p| p.1).sum();
            let mut times = vec![];
            for rep in 0..=REPEATS {
                let t = Instant::now();
                let mut vectors = vec![];
                for &(start, n) in &selections {
                    vectors.push(
                        r.read_vector_as::<rdigital_rf::Iq<i8>>(start as u64, n, "ch0", Some(0))?
                            .samples,
                    );
                }
                let elapsed = t.elapsed().as_secs_f64();
                for (&(start, _), v) in selections.iter().zip(&vectors) {
                    for (i, z) in v.iter().enumerate() {
                        let (a, b) = value(start + i, 8);
                        ensure!(z.r == a as i8 && z.i == b as i8, "typed i8 mismatch");
                    }
                }
                if rep > 0 {
                    times.push(elapsed);
                }
            }
            let mut sorted = times.clone();
            sorted.sort_by(f64::total_cmp);
            let median = sorted[REPEATS / 2];
            let name = format!(
                "{source}_i8_native_{}",
                if sparse { "sparse" } else { "contiguous" }
            );
            let g = report.create_group(&name)?;
            g.new_dataset_builder()
                .with_data(&times)
                .create("seconds")?;
            for (k, v) in [
                ("median_seconds", median),
                ("min_seconds", sorted[0]),
                ("max_seconds", sorted[REPEATS - 1]),
                ("million_samples_per_second", total as f64 / median / 1e6),
            ] {
                g.new_dataset::<f64>().create(k)?.write_scalar(&v)?;
            }
            for (k, v) in [
                ("samples_per_repeat", total as u64),
                ("stored_component_bits", 8),
                ("returned_sample_bytes", 2),
                ("fixture_bytes", disk_bytes(&root.join(source))?),
                ("exact_equal", 1),
            ] {
                g.new_dataset::<u64>().create(k)?.write_scalar(&v)?;
            }
            println!("{name} median_ms={:.3} exact=true", median * 1000.);
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .context("supply a new HDF5 report path")?;
    ensure!(!Path::new(&out).exists(), "report already exists");
    let tmp = tempfile::tempdir()?;
    for (name, bits, gzip) in [("hdf8", 8, false), ("gzip8", 8, true), ("hdf16", 16, false)] {
        fixture(&tmp.path().join(name), bits, gzip)?;
    }
    raw_fixture(&tmp.path().join("raw8"), 8)?;
    raw_fixture(&tmp.path().join("raw16"), 16)?;
    let report = File::create(out)?;
    text(&report,"method","Warm OS cache; per-case reader retained after one untimed warm-up; five timed repeats; output checked exactly outside timer")?;
    text(&report,"signal","Deterministic compressible staircase I/Q; i16 values are i8 values multiplied by 256; one channel/subchannel; one-second chunks")?;
    text(
        &report,
        "system",
        &std::process::Command::new("uname")
            .arg("-sm")
            .output()
            .map(|x| String::from_utf8_lossy(&x.stdout).trim().to_owned())
            .unwrap_or_default(),
    )?;
    attr(&report, "fixture_samples", N as u64)?;
    attr(&report, "repeats", REPEATS as u64)?;
    let cases = [
        ("hdf8_native", "hdf8", 8, false, None),
        ("hdf8_vector", "hdf8", 8, true, None),
        ("gzip8_default", "gzip8", 8, false, None),
        ("gzip8_128k_cache", "gzip8", 8, false, Some(131072)),
        ("hdf16_native", "hdf16", 16, false, None),
        ("raw8_native", "raw8", 8, false, None),
        ("raw8_vector", "raw8", 8, true, None),
        ("raw16_native", "raw16", 16, false, None),
    ];
    for (name, source, bits, vector, cap) in cases {
        for sparse in [false, true] {
            let pattern = if sparse { "sparse" } else { "contiguous" };
            let root = tmp.path().join(source);
            let mut r = Reader::open([&root])?;
            if let Some(cap) = cap {
                r.set_cache_byte_limit(cap)?;
            }
            let selections: Vec<_> = if sparse {
                (0..32).map(|i| (i * 65536 + 1024, 4096)).collect()
            } else {
                vec![(0, N)]
            };
            let total: usize = selections.iter().map(|p| p.1).sum();
            let mut times = vec![];
            for rep in 0..=REPEATS {
                let timer = Instant::now();
                let mut outputs = vec![];
                for &(start, n) in &selections {
                    outputs.push(load(&mut r, start, n, vector)?);
                }
                let elapsed = timer.elapsed().as_secs_f64();
                for (&(start, _), v) in selections.iter().zip(&outputs) {
                    for (i, z) in v.iter().enumerate() {
                        let (a, b) = value(start + i, bits);
                        ensure!(z.re == a && z.im == b, "sample mismatch in {name}");
                    }
                }
                if rep > 0 {
                    times.push(elapsed);
                }
            }
            let mut ordered = times.clone();
            ordered.sort_by(f64::total_cmp);
            let median = ordered[REPEATS / 2];
            let g = report.create_group(&format!("{name}_{pattern}"))?;
            g.new_dataset_builder()
                .with_data(&times)
                .create("seconds")?;
            for (k, v) in [
                ("median_seconds", median),
                ("min_seconds", ordered[0]),
                ("max_seconds", ordered[REPEATS - 1]),
                ("million_samples_per_second", total as f64 / median / 1e6),
            ] {
                g.new_dataset::<f64>().create(k)?.write_scalar(&v)?;
            }
            for (k, v) in [
                ("samples_per_repeat", total as u64),
                ("stored_component_bits", u64::from(bits)),
                ("fixture_bytes", disk_bytes(&root)?),
                ("exact_equal", 1),
                ("file_opens", r.cache_stats().file_opens),
                ("file_hits", r.cache_stats().file_hits),
            ] {
                g.new_dataset::<u64>().create(k)?.write_scalar(&v)?;
            }
            println!(
                "{name:20} {pattern:10} median_ms={:.3} MSps={:.2} exact=true",
                median * 1000.,
                total as f64 / median / 1e6
            );
        }
    }
    text(&report,"control_method","Contiguous HDF5 payload offset resolved before timing. Both payload files read using identical seek/read_exact into preallocated buffers; alternating order. Identical conversion routine reuses one preallocated output. No HDF5 API call or allocation inside payload control timer.")?;
    native_i8_cases(&report, tmp.path())?;
    audit(&report, tmp.path(), 8)?;
    audit(&report, tmp.path(), 16)?;
    report.flush()?;
    Ok(())
}
