use rdigital_rf::{
    raw::{Component, Container, Options, RawWriter},
    Reader,
};
use tempfile::TempDir;
fn options() -> Options {
    Options {
        sample_rate_numerator: 10,
        file_cadence_millisecs: 1000,
        subdir_cadence_secs: 2,
        ..Options::default()
    }
}
fn iq(a: &[i8]) -> Vec<u8> {
    a.iter()
        .flat_map(|&x| [x as u8, (-i16::from(x)).min(127) as i8 as u8])
        .collect()
}
#[test]
fn publication_gaps_rollover_and_vector_api() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let mut w = RawWriter::create(d.path().join("ch0"), options())?;
    assert_eq!(Container::default(), Container::Hdf5);
    w.write(18, &iq(&[-128, 127]))?; // published at second boundary
    let mut r = Reader::open([d.path()])?;
    assert_eq!(r.get_bounds("ch0")?, Some((18, 19)));
    w.write(20, &iq(&[1, 2]))?;
    w.write(24, &iq(&[3, 4]))?;
    // Live chunk invisible before commit, even to a reader opened beforehand.
    assert_eq!(r.bounds("ch0")?, Some((18, 19)));
    w.flush()?;
    assert_eq!(r.bounds("ch0")?, Some((18, 25)));
    let a = r.read_complex32(18, 4, "ch0", 0)?;
    assert_eq!(
        a.iter().map(|z| z.re).collect::<Vec<_>>(),
        vec![-128., 127., 1., 2.]
    );
    let v = r.read_vector(18, 4, "ch0", None)?.into_complex64();
    assert_eq!(v[0].re, -128.);
    assert_eq!(v[0].im, 127.);
    let g = r.read("ch0", 18..26, 0)?;
    assert_eq!(g.gaps, vec![22..24]);
    assert!(r.read_complex32(20, 6, "ch0", 0).is_err());
    assert!(w.write(25, &iq(&[7])).is_err());
    Ok(())
}
#[test]
fn ring_evicts_without_reusing_cached_files() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let mut o = options();
    o.ring_files = Some(2);
    let mut w = RawWriter::create(d.path().join("ch0"), o)?;
    w.write(0, &iq(&[1; 10]))?;
    let mut r = Reader::open([d.path()])?;
    assert_eq!(r.read_complex32(0, 1, "ch0", 0)?[0].re, 1.);
    w.write(10, &iq(&[2; 10]))?;
    w.write(20, &iq(&[3; 10]))?;
    let mut fresh = Reader::open([d.path()])?;
    assert_eq!(fresh.bounds("ch0")?, Some((10, 29)));
    assert!(fresh.read_vector(0, 1, "ch0", None).is_err());
    // An already-open inode remains usable across unlink; no unrelated data appears.
    assert_eq!(r.read_complex32(0, 1, "ch0", 0)?[0].re, 1.);
    r.clear_cache();
    assert!(r.read_complex32(0, 1, "ch0", 0).is_err());
    Ok(())
}
#[test]
fn scalar_types_and_subchannels() -> anyhow::Result<()> {
    for (component, bytes, expected) in [
        (Component::I8, vec![128, 127], vec![-128., 127.]),
        (
            Component::I16,
            [-32768i16, 32767]
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect(),
            vec![-32768., 32767.],
        ),
        (
            Component::F32,
            [-1.25f32, 2.5]
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect(),
            vec![-1.25, 2.5],
        ),
        (
            Component::F64,
            [-1.25f64, 2.5]
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect(),
            vec![-1.25, 2.5],
        ),
    ] {
        let d = TempDir::new()?;
        let mut o = options();
        o.component = component;
        o.complex = false;
        o.num_subchannels = 2;
        let mut w = RawWriter::create(d.path().join("ch0"), o)?;
        let payload: Vec<_> = bytes
            .iter()
            .copied()
            .cycle()
            .take(bytes.len() * 2)
            .collect();
        w.write(0, &payload)?;
        w.flush()?;
        let mut r = Reader::open([d.path()])?;
        let v = r.read_vector(0, 2, "ch0", None)?;
        assert_eq!(v.shape, vec![2, 2]);
        let v = v.into_complex64();
        assert_eq!(
            v.iter().map(|z| z.re).collect::<Vec<_>>(),
            vec![expected[0], expected[1], expected[0], expected[1]]
        );
        assert_eq!(r.read_complex32(0, 2, "ch0", 1)?[0].re, expected[1] as f32);
    }
    Ok(())
}
#[test]
fn truncated_payload_is_rejected_and_unfinished_chunk_is_hidden() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let mut w = RawWriter::create(d.path().join("ch0"), options())?;
    w.write(0, &iq(&[1; 10]))?;
    w.write(10, &iq(&[2; 3]))?;
    let mut r = Reader::open([d.path()])?;
    assert_eq!(r.bounds("ch0")?, Some((0, 9)));
    let p = d.path().join("ch0/1970-01-01T00-00-00/rf@0.000.raw");
    std::fs::OpenOptions::new()
        .write(true)
        .open(p)?
        .set_len(1)?;
    assert!(r.read_complex32(0, 1, "ch0", 0).is_err());
    Ok(())
}
#[test]
fn invalid_write_and_single_writer_are_rejected() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let p = d.path().join("ch0");
    let mut w = RawWriter::create(&p, options())?;
    assert!(RawWriter::create(&p, options()).is_err());
    assert!(w.write(0, &[1]).is_err());
    assert!(w.write(u64::MAX, &[1, 2]).is_err());
    w.write(100, &iq(&[1]))?;
    assert!(w.write(99, &iq(&[1])).is_err());
    Ok(())
}

#[test]
fn extreme_raw_timestamp_returns_error_without_wrapping() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let mut o = options();
    o.sample_rate_numerator = u64::MAX;
    o.sample_rate_denominator = u64::MAX;
    let mut w = RawWriter::create(d.path().join("ch0"), o)?;
    assert!(w.write(u64::MAX - 2, &[1, 2]).is_err());
    Ok(())
}
#[test]
fn ring_order_uses_numeric_seconds() -> anyhow::Result<()> {
    let d = TempDir::new()?;
    let mut o = options();
    o.ring_files = Some(2);
    o.subdir_cadence_secs = 3600;
    let mut w = RawWriter::create(d.path().join("ch0"), o)?;
    for s in [90, 100, 110] {
        w.write(s, &iq(&[1; 10]))?;
    }
    let mut r = Reader::open([d.path()])?;
    assert_eq!(r.bounds("ch0")?, Some((100, 119)));
    Ok(())
}
