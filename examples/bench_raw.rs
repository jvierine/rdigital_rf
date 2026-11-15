//! Bounded receiver-local comparison using identical archived signed-i8 samples.
use anyhow::{ensure, Result};
use rdigital_rf::{
    raw::{Options, RawWriter},
    Reader,
};
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().collect();
    ensure!(a.len() == 5, "SOURCE START COUNT NEW_RAW_CHANNEL");
    let start: u64 = a[2].parse()?;
    let n: usize = a[3].parse()?;
    ensure!(n > 0 && n <= 50_000_000, "bounded benchmark");
    let mut h = Reader::open([&a[1]])?;
    let props = h.properties("ch0")?.clone();
    let t = std::time::Instant::now();
    let expected = h.read_complex32(start, n, "ch0", 0)?;
    let hdf_s = t.elapsed().as_secs_f64();
    let mut bytes = Vec::with_capacity(n * 2);
    for z in &expected {
        for v in [z.re, z.im] {
            ensure!(
                v.is_finite() && v >= -128. && v <= 127. && v.fract() == 0.,
                "source not signed-i8"
            );
            bytes.push(v as i8 as u8);
        }
    }
    let mut w = RawWriter::create(
        &a[4],
        Options {
            sample_rate_numerator: props.sample_rate_numerator,
            sample_rate_denominator: props.sample_rate_denominator,
            ..Options::default()
        },
    )?;
    let t = std::time::Instant::now();
    w.write(start, &bytes)?;
    w.flush()?;
    let write_s = t.elapsed().as_secs_f64();
    drop(bytes);
    let mut r = Reader::open([&a[4]])?;
    let t = std::time::Instant::now();
    let actual = r.read_complex32(start, n, "ch0", 0)?;
    let raw_s = t.elapsed().as_secs_f64();
    ensure!(actual == expected, "container mismatch");
    println!("samples={n} hdf5_read_s={hdf_s:.6} raw_write_s={write_s:.6} raw_read_s={raw_s:.6} exact_equal=true");
    Ok(())
}
