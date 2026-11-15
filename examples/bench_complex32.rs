use anyhow::Result;
use rdigital_rf::Reader;
fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let mut r = Reader::open([&a[1]])?;
    let n: usize = a[3].parse()?;
    anyhow::ensure!(n <= 50_000_000, "bounded benchmark");
    let start = std::time::Instant::now();
    let v = r.read_complex32(a[2].parse()?, n, "ch0", 0)?;
    println!(
        "samples={} elapsed_s={:.6} checksum={:.1}",
        v.len(),
        start.elapsed().as_secs_f64(),
        v.iter()
            .map(|z| f64::from(z.re) + f64::from(z.im))
            .sum::<f64>()
    );
    Ok(())
}
