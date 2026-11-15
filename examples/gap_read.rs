use anyhow::{Context, Result};
use rdigital_rf::Reader;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = args.get(1).context("supply root, start and end")?;
    let start: u64 = args.get(2).context("supply start")?.parse()?;
    let end: u64 = args.get(3).context("supply end")?.parse()?;
    let mut reader = Reader::open([root])?;
    let result = reader.read("ch0", start..end, 0)?;
    for block in &result.blocks {
        println!(
            "block starts at {}: {} samples",
            block.start,
            block.data.len()
        );
    }
    for gap in &result.gaps {
        println!("gap: {}..{} (end excluded)", gap.start, gap.end);
    }
    // Strict consumers may reject the entire selection instead.
    if result.is_complete() {
        let samples = result.into_contiguous()?;
        println!("contiguous samples: {}", samples.len());
    }
    Ok(())
}
