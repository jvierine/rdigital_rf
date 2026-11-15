use anyhow::Result;
use rdigital_rf::{
    raw::{Component, Options, RawWriter},
    Reader,
};

fn main() -> Result<()> {
    // A disposable recording keeps this example safe to run repeatedly.
    let directory = tempfile::tempdir()?;
    let channel = directory.path().join("ch0");
    let options = Options {
        component: Component::I8,
        sample_rate_numerator: 50_000_000,
        ring_files: Some(40), // at most 40 finalized one-second chunks
        ..Options::default()
    };
    let mut writer = RawWriter::create(&channel, options)?;
    let start = 1_800_000_000u64 * 50_000_000;
    let iq: [i8; 8] = [-128, 127, -1, 1, 0, 0, 42, -42];
    let bytes: Vec<u8> = iq.iter().map(|&value| value as u8).collect();
    writer.write(start, &bytes)?; // four complex samples, not eight
    writer.flush()?; // publish this short chunk; do not append to its interval
    let mut reader = Reader::open([directory.path()])?;
    assert_eq!(reader.get_bounds("ch0")?, Some((start, start + 3)));
    let samples = reader.read_complex32(start, 4, "ch0", 0)?;
    assert_eq!(samples[0].re, -128.0);
    assert_eq!(samples[0].im, 127.0);
    println!("read {} exact raw-container samples", samples.len());
    Ok(())
}
