//! Diagnose persisted-session costs without altering the source snapshot.
use std::time::Instant;
use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_project::TimeUs;
fn main() {
    let path = std::env::args().nth(1).expect("snapshot path");
    let bytes = std::fs::read(path).unwrap();
    for _ in 0..3 {
        let began = Instant::now();
        let mut session = EditSession::restore(&bytes).unwrap();
        let restored = began.elapsed();
        assert!(
            session.snapshot().unwrap() == bytes,
            "recovery bytes changed"
        );
        let began = Instant::now();
        let cached_copy = session.clone();
        let cloned = began.elapsed();
        std::hint::black_box(cached_copy);
        let clip = &session.history().project().clips[0];
        let request = EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: session.history().revision(),
            action: EditAction::Apply {
                commands: vec![Command::MoveClip {
                    clip_id: clip.id.clone(),
                    start_us: TimeUs(clip.start_us.0 + 1),
                }],
            },
        };
        let began = Instant::now();
        session.execute(request).unwrap();
        let edited = began.elapsed();
        let began = Instant::now();
        let result = session.snapshot().unwrap();
        println!(
            "{}",
            serde_json::json!({"restore_ms":restored.as_secs_f64()*1000.,
            "clone_ms":cloned.as_secs_f64()*1000., "edit_ms":edited.as_secs_f64()*1000., "snapshot_ms":began.elapsed().as_secs_f64()*1000.,
            "input_bytes":bytes.len(),"output_bytes":result.len()})
        );
    }
}
