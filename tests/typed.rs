use anyhow::Result;
use hdf5::{types::VarLenUnicode, File};
use rdigital_rf::{
    raw::{Options, RawWriter},
    Iq, Reader, C128, C64,
};
use std::{path::Path, str::FromStr};
fn u64_fixture(root: &Path, values: &[u64]) -> Result<()> {
    let ch = root.join("ch0");
    std::fs::create_dir_all(ch.join("1970-01-01T00-00-00"))?;
    let h = File::create(ch.join("drf_properties.h5"))?;
    for (n, v) in [
        ("sample_rate_numerator", 10),
        ("sample_rate_denominator", 1),
        ("num_subchannels", 1),
        ("is_complex", 0),
        ("file_cadence_millisecs", 1000),
        ("subdir_cadence_secs", 3600),
        ("H5Tget_class", 0),
        ("H5Tget_size", 8),
    ] {
        h.new_attr::<u64>().create(n)?.write_scalar(&v)?;
    }
    for (n, v) in [
        ("digital_rf_version", "2.6.0"),
        ("epoch", "1970-01-01T00:00:00Z"),
    ] {
        h.new_attr::<VarLenUnicode>()
            .create(n)?
            .write_scalar(&VarLenUnicode::from_str(v)?)?;
    }
    let h = File::create(ch.join("1970-01-01T00-00-00/rf@0.000.h5"))?;
    h.new_dataset::<u64>()
        .shape((values.len(), 1))
        .create("rf_data")?
        .write_raw(values)?;
    h.new_dataset::<u64>()
        .shape((1, 2))
        .create("rf_data_index")?
        .write_raw(&[0, 0])?;
    Ok(())
}
#[test]
fn integer_u64_reads_do_not_round_through_float64() -> Result<()> {
    let d = tempfile::tempdir()?;
    let values = [(1u64 << 53) + 1, (1u64 << 63) + 3, u64::MAX];
    u64_fixture(d.path(), &values)?;
    let mut r = Reader::open([d.path()])?;
    let v = r.read_vector_as::<u64>(0, 3, "ch0", Some(0))?;
    assert_eq!(v.samples, values);
    assert_eq!(v.shape, vec![3]);
    assert_eq!(r.read_as::<u64>("ch0", 0..3, 0)?.into_contiguous()?, values);
    assert!(r.read_vector_as::<i8>(0, 3, "ch0", Some(0)).is_err());
    Ok(())
}
#[test]
fn integer_overlap_conflicts_are_not_hidden_by_float_rounding() -> Result<()> {
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    u64_fixture(a.path(), &[(1u64 << 53) + 3])?;
    u64_fixture(b.path(), &[(1u64 << 53) + 4])?;
    let mut r = Reader::open([a.path(), b.path()])?;
    assert!(r.read_vector_as::<u64>(0, 1, "ch0", Some(0)).is_err());
    Ok(())
}
#[test]
fn raw_complex_i8_and_complex64_have_requested_sizes_and_values() -> Result<()> {
    assert_eq!(std::mem::size_of::<Iq<i8>>(), 2);
    assert_eq!(std::mem::size_of::<C64>(), 8);
    assert_eq!(std::mem::size_of::<C128>(), 16);
    let d = tempfile::tempdir()?;
    let mut w = RawWriter::create(
        d.path().join("ch0"),
        Options {
            sample_rate_numerator: 10,
            ..Options::default()
        },
    )?;
    w.write(0, &[128, 127, 255, 1, 42, 214])?;
    w.flush()?;
    let mut r = Reader::open([d.path()])?;
    let v = r.read_vector_as::<Iq<i8>>(0, 3, "ch0", Some(0))?;
    assert_eq!(
        v.samples,
        vec![
            Iq { r: -128, i: 127 },
            Iq { r: -1, i: 1 },
            Iq { r: 42, i: -42 }
        ]
    );
    assert_eq!(v.samples.capacity(), 3);
    let z = r.read_vector_as::<C64>(0, 3, "ch0", Some(0))?;
    assert_eq!(z.samples[0], C64::new(-128., 127.));
    assert!(r.read_vector_as::<i8>(0, 3, "ch0", Some(0)).is_err());
    assert!(r.read_vector_as::<Iq<i16>>(0, 3, "ch0", Some(0)).is_err());
    Ok(())
}
#[test]
fn typed_multichannel_raw_selection_and_gaps() -> Result<()> {
    let d = tempfile::tempdir()?;
    let mut w = RawWriter::create(
        d.path().join("ch0"),
        Options {
            sample_rate_numerator: 10,
            num_subchannels: 2,
            ..Options::default()
        },
    )?;
    w.write(0, &[1, 2, 3, 4, 5, 6, 7, 8])?;
    w.write(4, &[9, 10, 11, 12])?;
    w.flush()?;
    let mut r = Reader::open([d.path()])?;
    let all = r.read_vector_as::<Iq<i8>>(0, 2, "ch0", None)?;
    let one = r.read_vector_as::<Iq<i8>>(0, 1, "ch0", None)?;
    assert_eq!(one.shape, vec![1, 2]);
    assert_eq!(one.samples.len(), 2);
    assert!(r.read_vector(0, 1, "ch0", None).is_err());
    assert_eq!(all.shape, vec![2, 2]);
    assert_eq!(
        all.samples,
        vec![
            Iq { r: 1, i: 2 },
            Iq { r: 3, i: 4 },
            Iq { r: 5, i: 6 },
            Iq { r: 7, i: 8 }
        ]
    );
    let second = r.read_vector_as::<C64>(0, 2, "ch0", Some(1))?;
    assert_eq!(second.samples, vec![C64::new(3., 4.), C64::new(7., 8.)]);
    let gaps = r.read_as::<Iq<i8>>("ch0", 0..5, 1)?;
    assert_eq!(gaps.gaps, vec![2..4]);
    assert_eq!(gaps.blocks[1].data, vec![Iq { r: 11, i: 12 }]);
    assert!(r.read_vector_as::<Iq<i8>>(0, 5, "ch0", Some(1)).is_err());
    Ok(())
}

// Track allocations on this test thread only; the native read must allocate
// its 8192-byte result, not an expanded sample array or fixed scratch buffer.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    static TRACK:Cell<bool>=const {Cell::new(false)};
    static LARGEST:Cell<usize>=const {Cell::new(0)};
    static TOTAL:Cell<usize>=const {Cell::new(0)};
}
struct Allocations;
fn record(n: usize) {
    if TRACK.try_with(|v| v.get()).unwrap_or(false) {
        let _ = LARGEST.try_with(|v| v.set(v.get().max(n)));
        let _ = TOTAL.try_with(|v| v.set(v.get() + n));
    }
}
unsafe impl GlobalAlloc for Allocations {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record(n);
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATIONS: Allocations = Allocations;
#[test]
fn native_i8_raw_read_allocates_only_requested_sample_storage() -> Result<()> {
    let d = tempfile::tempdir()?;
    let mut w = RawWriter::create(d.path().join("ch0"), Options::default())?;
    w.write(0, &vec![127u8; 8192])?;
    w.flush()?;
    let mut r = Reader::open([d.path()])?;
    r.bounds("ch0")?;
    LARGEST.with(|v| v.set(0));
    TOTAL.with(|v| v.set(0));
    TRACK.with(|v| v.set(true));
    let result = r.read_vector_as::<Iq<i8>>(0, 4096, "ch0", Some(0));
    TRACK.with(|v| v.set(false));
    let got = result?;
    assert_eq!(got.samples.len(), 4096);
    assert_eq!(got.samples.capacity(), 4096);
    assert!(
        LARGEST.with(|v| v.get()) <= 8192,
        "unexpected expanded or fixed scratch allocation"
    );
    assert!(
        TOTAL.with(|v| v.get()) <= 8192 + 4096,
        "unexpected second sample buffer"
    );
    LARGEST.with(|v| v.set(0));
    TOTAL.with(|v| v.set(0));
    TRACK.with(|v| v.set(true));
    let result = r.read_vector_as::<C64>(0, 4096, "ch0", Some(0));
    TRACK.with(|v| v.set(false));
    let got = result?;
    assert_eq!(got.samples.len(), 4096);
    assert!(
        LARGEST.with(|v| v.get()) <= 4096 * 8,
        "unexpected float64 staging buffer"
    );
    assert!(
        TOTAL.with(|v| v.get()) <= 4096 * 8 + 8192 + 4096,
        "unexpected conversion buffer size"
    );
    Ok(())
}

#[test]
fn native_all_subchannels_read_directly_into_row_major_result() -> Result<()> {
    let d = tempfile::tempdir()?;
    let mut w = RawWriter::create(
        d.path().join("ch0"),
        Options {
            num_subchannels: 2,
            ..Options::default()
        },
    )?;
    w.write(0, &vec![127u8; 8192])?;
    w.flush()?;
    let mut r = Reader::open([d.path()])?;
    r.bounds("ch0")?;
    LARGEST.with(|v| v.set(0));
    TOTAL.with(|v| v.set(0));
    TRACK.with(|v| v.set(true));
    let result = r.read_vector_as::<Iq<i8>>(0, 2048, "ch0", None);
    TRACK.with(|v| v.set(false));
    let got = result?;
    assert_eq!(got.shape, vec![2048, 2]);
    assert_eq!(got.samples.len(), 4096);
    assert_eq!(got.samples.capacity(), 4096);
    assert!(LARGEST.with(|v| v.get()) <= 8192);
    assert!(
        TOTAL.with(|v| v.get()) <= 8192 + 4096,
        "unnecessary multichannel sample buffer"
    );
    Ok(())
}
