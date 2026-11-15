//! Optional raw-container extension, version 1. Raw payloads are little-endian,
//! row-major (sample, subchannel, component). HDF5 contains metadata only.
use crate::{attr_u64, Complex64};
use anyhow::{ensure, Context, Result};
use hdf5::File;
#[cfg(not(unix))]
use std::io::{Read, Seek, SeekFrom};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Container {
    #[default]
    Hdf5,
    Raw,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Component {
    I8 = 1,
    I16 = 2,
    F32 = 3,
    F64 = 4,
}
impl Component {
    fn from_id(n: u64) -> Result<Self> {
        Ok(match n {
            1 => Self::I8,
            2 => Self::I16,
            3 => Self::F32,
            4 => Self::F64,
            _ => anyhow::bail!("unsupported raw component"),
        })
    }
    pub fn bytes(self) -> usize {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }
    fn value(self, b: &[u8]) -> f64 {
        match self {
            Self::I8 => (b[0] as i8) as f64,
            Self::I16 => i16::from_le_bytes(b[..2].try_into().unwrap()) as f64,
            Self::F32 => f32::from_le_bytes(b[..4].try_into().unwrap()) as f64,
            Self::F64 => f64::from_le_bytes(b[..8].try_into().unwrap()),
        }
    }
}
fn attr(h: &File, n: &str, v: u64) -> Result<()> {
    h.new_attr::<u64>().create(n)?.write_scalar(&v)?;
    Ok(())
}
fn text(h: &File, n: &str, v: &str) -> Result<()> {
    use std::str::FromStr;
    h.new_attr::<hdf5::types::VarLenUnicode>()
        .create(n)?
        .write_scalar(&hdf5::types::VarLenUnicode::from_str(v)?)?;
    Ok(())
}

pub(crate) struct Chunk {
    file: fs::File,
    component: Component,
    complex: bool,
    ns: usize,
    pub index: Vec<(u64, u64)>,
    pub rows: u64,
}
impl Chunk {
    pub fn open(path: &Path, h: &File, ns: usize) -> Result<Self> {
        ensure!(
            attr_u64(h, "rdigital_rf_raw_version")? == 1,
            "unsupported raw version"
        );
        let component = Component::from_id(attr_u64(h, "raw_component")?)?;
        let complex = attr_u64(h, "raw_is_complex")? != 0;
        ensure!(
            attr_u64(h, "raw_num_subchannels")? == ns as u64,
            "raw subchannel mismatch"
        );
        let rows = attr_u64(h, "raw_rows")?;
        let ds = h.dataset("rf_data_index")?;
        ensure!(
            ds.shape().len() == 2 && ds.shape()[1] == 2,
            "invalid raw index shape"
        );
        let index: Vec<_> = ds
            .read_raw::<u64>()?
            .chunks_exact(2)
            .map(|p| (p[0], p[1]))
            .collect();
        ensure!(
            !index.is_empty() && index[0].1 == 0,
            "missing raw first block"
        );
        for (i, &(s, o)) in index.iter().enumerate() {
            let end = index.get(i + 1).map_or(rows, |p| p.1);
            ensure!(o < end && end <= rows, "invalid raw block offsets");
            let stop = s.checked_add(end - o).context("raw sample overflow")?;
            if let Some(next) = index.get(i + 1) {
                ensure!(stop <= next.0, "overlapping raw blocks");
            }
        }
        let file = fs::File::open(path.with_extension("raw"))?;
        let bytes = rows
            .checked_mul(ns as u64)
            .and_then(|v| v.checked_mul(if complex { 2 } else { 1 }))
            .and_then(|v| v.checked_mul(component.bytes() as u64))
            .context("raw length overflow")?;
        ensure!(
            file.metadata()?.len() == bytes,
            "raw payload length mismatch"
        );
        Ok(Self {
            file,
            component,
            complex,
            ns,
            index,
            rows,
        })
    }
    fn read_values<T>(
        &self,
        a: usize,
        b: usize,
        ch: usize,
        mut convert: impl FnMut(f64, f64) -> T,
    ) -> Result<Vec<T>> {
        ensure!(
            a <= b && b as u64 <= self.rows && ch < self.ns,
            "invalid raw selection"
        );
        let nc = if self.complex { 2 } else { 1 };
        let size = self.component.bytes();
        let stride = self
            .ns
            .checked_mul(nc * size)
            .context("raw stride overflow")?;
        let offset = (a as u64)
            .checked_mul(stride as u64)
            .context("raw offset overflow")?;
        let mut file = self.file.try_clone()?;
        // Positional reads avoid a shared seek cursor between reader threads.
        let mut out = Vec::with_capacity(b - a);
        let chunk_rows = (1024 * 1024 / stride).max(1).min((b - a).max(1));
        let mut buf = vec![
            0u8;
            chunk_rows
                .checked_mul(stride)
                .context("raw buffer overflow")?
        ];
        let mut row = 0;
        while row < b - a {
            let n = (b - a - row).min(chunk_rows);
            let bytes = n * stride;
            read_at(&mut file, &mut buf[..bytes], offset + (row * stride) as u64)?;
            for r in buf[..bytes].chunks_exact(stride) {
                let i = ch * nc * size;
                let re = self.component.value(&r[i..]);
                let im = if self.complex {
                    self.component.value(&r[i + size..])
                } else {
                    0.
                };
                out.push(convert(re, im));
            }
            row += n;
        }
        Ok(out)
    }
    fn packed32<const STRIDE: usize>(
        &self,
        a: usize,
        b: usize,
        convert: impl Fn(&[u8]) -> num_complex::Complex32,
        out: &mut Vec<num_complex::Complex32>,
    ) -> Result<()> {
        ensure!(a <= b && b as u64 <= self.rows, "invalid raw selection");
        let byte_count = (b - a)
            .checked_mul(STRIDE)
            .context("raw byte count overflow")?;
        let offset = (a as u64)
            .checked_mul(STRIDE as u64)
            .context("raw offset overflow")?;
        let mut bytes = vec![0u8; byte_count];
        let mut file = self.file.try_clone()?;
        read_at(&mut file, &mut bytes, offset)?;
        out.extend(bytes.chunks_exact(STRIDE).map(convert));
        Ok(())
    }
    pub fn append32(
        &self,
        a: usize,
        b: usize,
        ch: usize,
        out: &mut Vec<num_complex::Complex32>,
    ) -> Result<()> {
        if self.ns == 1 && self.complex {
            ensure!(ch == 0, "invalid raw subchannel");
            use num_complex::Complex32 as Z;
            return match self.component {
                Component::I8 => {
                    self.packed32::<2>(a, b, |v| Z::new(v[0] as i8 as f32, v[1] as i8 as f32), out)
                }
                Component::I16 => self.packed32::<4>(
                    a,
                    b,
                    |v| {
                        Z::new(
                            i16::from_le_bytes([v[0], v[1]]) as f32,
                            i16::from_le_bytes([v[2], v[3]]) as f32,
                        )
                    },
                    out,
                ),
                Component::F32 => self.packed32::<8>(
                    a,
                    b,
                    |v| {
                        Z::new(
                            f32::from_le_bytes(v[..4].try_into().unwrap()),
                            f32::from_le_bytes(v[4..].try_into().unwrap()),
                        )
                    },
                    out,
                ),
                Component::F64 => self.packed32::<16>(
                    a,
                    b,
                    |v| {
                        Z::new(
                            f64::from_le_bytes(v[..8].try_into().unwrap()) as f32,
                            f64::from_le_bytes(v[8..].try_into().unwrap()) as f32,
                        )
                    },
                    out,
                ),
            };
        }
        out.extend(self.read_values(a, b, ch, |r, i| {
            num_complex::Complex32::new(r as f32, i as f32)
        })?);
        Ok(())
    }
    pub fn read64(&self, a: usize, b: usize, ch: usize) -> Result<Vec<Complex64>> {
        self.read_values(a, b, ch, Complex64::new)
    }
}
#[cfg(unix)]
fn read_at(file: &mut fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}
#[cfg(not(unix))]
fn read_at(file: &mut fs::File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(buf)
}

/// Raw writing is opt-in. Existing HDF5 recording and reading remain unchanged.
#[derive(Clone, Debug)]
pub struct Options {
    pub component: Component,
    pub complex: bool,
    pub num_subchannels: usize,
    pub sample_rate_numerator: u64,
    pub sample_rate_denominator: u64,
    pub file_cadence_millisecs: u64,
    pub subdir_cadence_secs: u64,
    /// None retains all finalized chunks; Some(n) is a bounded file-count ring.
    pub ring_files: Option<usize>,
    /// Sync payload and metadata before publishing. Usually false for a RAM ring.
    pub durable: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            component: Component::I8,
            complex: true,
            num_subchannels: 1,
            sample_rate_numerator: 25_000_000,
            sample_rate_denominator: 1,
            file_cadence_millisecs: 1000,
            subdir_cadence_secs: 3600,
            ring_files: None,
            durable: false,
        }
    }
}
struct Pending {
    path: PathBuf,
    file: fs::File,
    ms: u64,
    rows: u64,
    index: Vec<(u64, u64)>,
}
/// Single-writer raw ring/archive. Chunks become visible only at cadence rollover
/// or explicit `flush`; dropping a writer does not publish a partial chunk.
pub struct RawWriter {
    path: PathBuf,
    options: Options,
    pending: Option<Pending>,
    next: Option<u64>,
    _lock: fs::File,
}
impl RawWriter {
    pub fn create(channel: impl AsRef<Path>, options: Options) -> Result<Self> {
        let o = &options;
        ensure!(
            o.num_subchannels > 0
                && o.sample_rate_numerator > 0
                && o.sample_rate_denominator > 0
                && o.file_cadence_millisecs > 0
                && o.subdir_cadence_secs > 0
                && o.ring_files != Some(0),
            "invalid raw writer options"
        );
        // Align all chunk boundaries to integer samples; no rounded timestamp loss.
        ensure!(
            (u128::from(o.sample_rate_numerator) * u128::from(o.file_cadence_millisecs))
                % (u128::from(o.sample_rate_denominator) * 1000)
                == 0,
            "raw cadence must contain an integer sample count"
        );
        ensure!(
            o.subdir_cadence_secs
                .checked_mul(1000)
                .context("cadence overflow")?
                % o.file_cadence_millisecs
                == 0,
            "file cadence must divide subdirectory cadence"
        );
        let path = channel.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;
        let lock = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.join(".raw-writer.lock"))
            .context("raw writer already exists; inspect stale lock after a crash")?;
        let result = (|| -> Result<()> {
            let props = path.join("drf_properties.h5");
            ensure!(
                !props.exists(),
                "raw writer requires a new channel directory"
            );
            let temp = path.join("drf_properties.h5.partial");
            let h = File::create(&temp)?;
            text(&h, "digital_rf_version", "2.6.0")?;
            text(&h, "epoch", "1970-01-01T00:00:00Z")?;
            for (n, v) in [
                ("sample_rate_numerator", o.sample_rate_numerator),
                ("sample_rate_denominator", o.sample_rate_denominator),
                ("num_subchannels", o.num_subchannels as u64),
                ("is_complex", o.complex as u64),
                (
                    "H5Tget_class",
                    if matches!(o.component, Component::I8 | Component::I16) {
                        0
                    } else {
                        1
                    },
                ),
                ("H5Tget_size", o.component.bytes() as u64),
                ("file_cadence_millisecs", o.file_cadence_millisecs),
                ("subdir_cadence_secs", o.subdir_cadence_secs),
                ("rdigital_rf_raw_version", 1),
            ] {
                attr(&h, n, v)?;
            }
            h.flush()?;
            drop(h);
            fs::rename(temp, props)?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(path.join(".raw-writer.lock"));
            return Err(e);
        }
        Ok(Self {
            path,
            options,
            pending: None,
            next: None,
            _lock: lock,
        })
    }
    /// Interleaved little-endian bytes, in (sample, subchannel, real/imag) order.
    /// Gaps are indexed explicitly. Overlapping/backward writes are rejected.
    pub fn write(&mut self, start: u64, mut bytes: &[u8]) -> Result<()> {
        let o = &self.options;
        let stride = o
            .num_subchannels
            .checked_mul(if o.complex { 2 } else { 1 })
            .and_then(|n| n.checked_mul(o.component.bytes()))
            .context("raw stride overflow")?;
        ensure!(
            !bytes.is_empty() && bytes.len() % stride == 0,
            "unaligned raw write"
        );
        let end = start
            .checked_add((bytes.len() / stride) as u64)
            .context("sample overflow")?;
        ensure!(
            self.next.is_none_or(|n| start >= n),
            "overlapping/backward raw write"
        );
        let mut sample = start;
        while !bytes.is_empty() {
            let o = &self.options;
            let ms = u64::try_from(
                u128::from(sample)
                    .checked_mul(u128::from(o.sample_rate_denominator))
                    .and_then(|n| n.checked_mul(1000))
                    .context("raw timestamp overflow")?
                    / u128::from(o.sample_rate_numerator),
            )? / o.file_cadence_millisecs
                * o.file_cadence_millisecs;
            if self.pending.as_ref().is_some_and(|p| p.ms != ms) {
                self.flush()?;
            }
            let o = &self.options;
            let boundary = u64::try_from(
                u128::from(
                    ms.checked_add(o.file_cadence_millisecs)
                        .context("time overflow")?,
                ) * u128::from(o.sample_rate_numerator)
                    / (u128::from(o.sample_rate_denominator) * 1000),
            )?;
            let n = usize::try_from((boundary - sample).min((bytes.len() / stride) as u64))?;
            ensure!(n > 0, "invalid raw cadence boundary");
            if self.pending.is_none() {
                let sec = ms / 1000 / o.subdir_cadence_secs * o.subdir_cadence_secs;
                let date = chrono::DateTime::<chrono::Utc>::from_timestamp(i64::try_from(sec)?, 0)
                    .context("timestamp outside calendar")?;
                let dir = self.path.join(date.format("%Y-%m-%dT%H-%M-%S").to_string());
                fs::create_dir_all(&dir)?;
                let path = dir.join(format!("rf@{}.{:03}.h5", ms / 1000, ms % 1000));
                ensure!(
                    !path.exists() && !path.with_extension("raw").exists(),
                    "raw chunk already published"
                );
                let file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path.with_extension("raw.partial"))?;
                self.pending = Some(Pending {
                    path,
                    file,
                    ms,
                    rows: 0,
                    index: vec![],
                });
            }
            let p = self.pending.as_mut().unwrap();
            if p.index
                .last()
                .is_none_or(|&(s, off)| s + (p.rows - off) != sample)
            {
                p.index.push((sample, p.rows));
            }
            p.file.write_all(&bytes[..n * stride])?;
            p.rows += n as u64;
            bytes = &bytes[n * stride..];
            sample += n as u64;
            self.next = Some(sample);
            if sample == boundary {
                self.flush()?;
            }
        }
        debug_assert_eq!(self.next, Some(end));
        Ok(())
    }
    /// Finalize current chunk. A finalized cadence interval cannot be appended.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(p) = self.pending.as_mut() {
            p.file.flush()?;
            if self.options.durable {
                p.file.sync_all()?;
            }
            let temp = p.path.with_extension("h5.partial");
            let h = File::create(&temp)?;
            for (n, v) in [
                ("rdigital_rf_raw_version", 1),
                ("raw_component", self.options.component as u64),
                ("raw_is_complex", self.options.complex as u64),
                ("raw_num_subchannels", self.options.num_subchannels as u64),
                ("raw_rows", p.rows),
            ] {
                attr(&h, n, v)?;
            }
            let index: Vec<_> = p.index.iter().flat_map(|&(s, o)| [s, o]).collect();
            h.new_dataset::<u64>()
                .shape((p.index.len(), 2))
                .create("rf_data_index")?
                .write_raw(&index)?;
            h.flush()?;
            drop(h);
            if self.options.durable {
                fs::File::open(&temp)?.sync_all()?;
            }
            fs::rename(
                p.path.with_extension("raw.partial"),
                p.path.with_extension("raw"),
            )?;
            fs::rename(temp, &p.path)?;
            if self.options.durable {
                fs::File::open(p.path.parent().unwrap())?.sync_all()?;
            }
            self.pending = None;
        }
        self.prune()
    }
    fn prune(&self) -> Result<()> {
        let Some(keep) = self.options.ring_files else {
            return Ok(());
        };
        let mut files = vec![];
        for d in fs::read_dir(&self.path)? {
            let dir = d?.path();
            if !dir.is_dir() {
                continue;
            }
            for f in fs::read_dir(dir)? {
                let p = f?.path();
                let n = p.file_name().unwrap().to_string_lossy();
                if n.starts_with("rf@") && n.ends_with(".h5") {
                    files.push(p);
                }
            }
        }
        files.sort_by_key(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.strip_prefix("rf@")
                .and_then(|n| n.strip_suffix(".h5"))
                .and_then(|n| n.split_once('.'))
                .and_then(|(s, m)| Some((s.parse::<u64>().ok()?, m.parse::<u64>().ok()?)))
                .unwrap_or((0, 0))
        });
        let count = files.len().saturating_sub(keep);
        for p in files.into_iter().take(count) {
            // Unpublish the commit marker first; an open reader retains its inode.
            fs::remove_file(&p)?;
            match fs::remove_file(p.with_extension("raw")) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            };
        }
        Ok(())
    }
}
impl Drop for RawWriter {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.path.join(".raw-writer.lock"));
    }
}
