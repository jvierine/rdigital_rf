use anyhow::{Context, Result};
use rdigital_rf::{CacheOptions, MissingSamples, Reader};

fn main() -> Result<()> {
    let root = std::env::args().nth(1).context("supply a recording root")?;
    let channel = std::env::args().nth(2).unwrap_or_else(|| "ch0".into());
    let mut reader = Reader::open([root])?;
    reader.set_cache_options(CacheOptions {
        minimum_bytes: 8 * 1024 * 1024,
        max_open_files: 2,
    })?;
    reader.set_cache_byte_limit(32 * 1024 * 1024)?;
    let Some((first, last)) = reader.get_bounds(&channel)? else {
        return Ok(());
    };
    let count = (last - first).min(1023) as usize + 1;
    match reader.read_complex32(first, count, &channel, 0) {
        Ok(iq) => println!("read {} complex samples", iq.len()),
        Err(error) => {
            if let Some(missing) = error.downcast_ref::<MissingSamples>() {
                for gap in &missing.gaps {
                    eprintln!("missing samples {}..{}", gap.start, gap.end);
                }
            } else {
                return Err(error);
            }
        }
    }
    Ok(())
}
