//! Bounded, read-only numeric Digital Metadata access. Ranges are half-open.
use anyhow::{bail, ensure, Context, Result};
use hdf5::{types::TypeDescriptor, File};
use std::{
    collections::BTreeMap,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq)]
pub enum Numeric {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
}
#[derive(Debug, PartialEq)]
pub struct Array {
    pub shape: Vec<usize>,
    pub values: Numeric,
}
pub type Record = BTreeMap<String, Array>;

/// One channel, no recursive archive scan and no writes. Numeric fields only;
/// unsupported types and missing requested fields are errors, never omissions.
pub struct Reader {
    root: PathBuf,
    numerator: u64,
    denominator: u64,
    file_seconds: u64,
    directory_seconds: u64,
    prefix: String,
}
impl Reader {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let h = File::open(root.as_ref().join("dmd_properties.h5"))?;
        let r = Self {
            root: root.as_ref().to_path_buf(),
            numerator: super::attr_u64(&h, "sample_rate_numerator")?,
            denominator: super::attr_u64(&h, "sample_rate_denominator")?,
            file_seconds: super::attr_u64(&h, "file_cadence_secs")?,
            directory_seconds: super::attr_u64(&h, "subdir_cadence_secs")?,
            prefix: super::attr_text(&h, "file_name")?,
        };
        ensure!(r.numerator > 0 && r.denominator > 0, "invalid sample rate");
        ensure!(
            r.file_seconds > 0
                && r.directory_seconds > 0
                && r.directory_seconds.is_multiple_of(r.file_seconds),
            "invalid metadata cadence"
        );
        ensure!(
            !r.prefix.is_empty() && !r.prefix.contains(['/', '\\']),
            "invalid filename prefix"
        );
        Ok(r)
    }

    pub fn sample_rate(&self) -> (u64, u64) {
        (self.numerator, self.denominator)
    }

    pub fn read(&self, samples: Range<u64>, fields: &[&str]) -> Result<BTreeMap<u64, Record>> {
        ensure!(samples.start <= samples.end, "reversed sample range");
        ensure!(!fields.is_empty(), "explicit field selection required");
        let mut result = BTreeMap::new();
        if samples.is_empty() {
            return Ok(result);
        }
        let second = |sample: u64| -> Result<u64> {
            Ok(u64::try_from(
                u128::from(sample) * u128::from(self.denominator) / u128::from(self.numerator),
            )?)
        };
        let start = second(samples.start)? / self.file_seconds;
        let stop = second(samples.end - 1)? / self.file_seconds;
        ensure!(
            stop - start < 100_000,
            "read too broad; request smaller time windows"
        );
        for slot in start..=stop {
            let seconds = slot
                .checked_mul(self.file_seconds)
                .context("timestamp overflow")?;
            let directory = seconds / self.directory_seconds * self.directory_seconds;
            let date = chrono::DateTime::from_timestamp(i64::try_from(directory)?, 0)
                .context("timestamp outside calendar range")?;
            let path = self
                .root
                .join(date.format("%Y-%m-%dT%H-%M-%S").to_string())
                .join(format!("{}@{}.h5", self.prefix, seconds));
            if !path.try_exists()? {
                continue;
            }
            let h = File::open(&path).with_context(|| format!("open {}", path.display()))?;
            for name in h.member_names()? {
                let sample: u64 = name
                    .parse()
                    .with_context(|| format!("invalid sample key {name}"))?;
                if !samples.contains(&sample) {
                    continue;
                }
                let group = h.group(&name)?;
                let mut record = Record::new();
                for field in fields {
                    let ds = group
                        .dataset(field)
                        .with_context(|| format!("sample {sample}, field {field}"))?;
                    use hdf5::types::{FloatSize as F, IntSize as I};
                    let values = match ds.dtype()?.to_descriptor()? {
                        TypeDescriptor::Integer(I::U1) => Numeric::I8(ds.read_raw()?),
                        TypeDescriptor::Integer(I::U2) => Numeric::I16(ds.read_raw()?),
                        TypeDescriptor::Integer(I::U4) => Numeric::I32(ds.read_raw()?),
                        TypeDescriptor::Integer(I::U8) => Numeric::I64(ds.read_raw()?),
                        TypeDescriptor::Unsigned(I::U1) => Numeric::U8(ds.read_raw()?),
                        TypeDescriptor::Unsigned(I::U2) => Numeric::U16(ds.read_raw()?),
                        TypeDescriptor::Unsigned(I::U4) => Numeric::U32(ds.read_raw()?),
                        TypeDescriptor::Unsigned(I::U8) => Numeric::U64(ds.read_raw()?),
                        TypeDescriptor::Float(F::U4) => Numeric::F32(ds.read_raw()?),
                        TypeDescriptor::Float(F::U8) => Numeric::F64(ds.read_raw()?),
                        other => {
                            bail!("unsupported numeric metadata type {other:?} at {sample}/{field}")
                        }
                    };
                    record.insert(
                        (*field).to_owned(),
                        Array {
                            shape: ds.shape(),
                            values,
                        },
                    );
                }
                ensure!(
                    result.insert(sample, record).is_none(),
                    "duplicate sample {sample}"
                );
            }
        }
        Ok(result)
    }
}
