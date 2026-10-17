use super::*;

#[test]
fn engine_manifest_exceeds_old_limit_without_omitting_last_dependency() {
    let (fixture, binding, _, _, _) = setup();
    let mut engine = binding.engine.clone();
    let source = fixture.0.join("dependency.py");
    std::fs::write(&source, "source").unwrap();
    for index in 0..45000 {
        engine
            .files
            .insert(format!("runtime/{index:05}"), source.clone());
    }
    let cancel = AtomicBool::new(false);
    let original = engine.revision(&cancel).unwrap();
    let last = fixture.0.join("last.py");
    std::fs::write(&last, "changed").unwrap();
    engine.files.insert("runtime/44999".into(), last);
    assert_ne!(engine.revision(&cancel).unwrap(), original);
    for index in 45000..65533 {
        engine
            .files
            .insert(format!("runtime/{index:05}"), source.clone());
    }
    assert_eq!(engine.files.len(), 65537);
    assert_eq!(
        engine.revision(&cancel).err().unwrap().code,
        ErrorCode::ValidationFailed
    );
}
