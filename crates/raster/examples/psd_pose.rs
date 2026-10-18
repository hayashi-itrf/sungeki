fn main() -> sungeki_raster::Result<()> {
    let a: Vec<_> = std::env::args().collect();
    let bytes = std::fs::read(&a[1])?;
    let doc = sungeki_raster::psd::Document::read(&bytes, &a[2])?;
    println!("{}", serde_json::to_string(&doc.probe)?);
    if a.len() > 4 {
        let pose = serde_json::from_slice(&std::fs::read(&a[3])?)?;
        doc.render(&pose)?.save(&a[4])?;
    }
    Ok(())
}
