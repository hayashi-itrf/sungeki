//! Measure potential fixed-block reuse on real editor snapshots. No database writes.
//! This measures hash matches, not a cache implementation or GUI acceptance.
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path, time::Instant};
use sungeki_editor::{Command, EditAction, EditRequest, EditSession};
use sungeki_storage::load_document;

fn edit(session: &mut EditSession, index: usize, displacement: u64) {
    let clip = &session.history().project().clips[index];
    let command = Command::MoveClip {
        clip_id: clip.id.clone(),
        start_us: sungeki_project::TimeUs(clip.start_us.0 + displacement),
    };
    session
        .execute(EditRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: session.history().revision(),
            action: EditAction::Apply {
                commands: vec![command],
            },
        })
        .unwrap();
}
fn main() {
    let source = std::env::args().nth(1).expect("PROJECT");
    let output = std::env::args().nth(2).map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir(path).unwrap();
    }
    let project = load_document(Path::new(&source)).unwrap().project;
    assert!(project.clips.len() >= 80);
    let mut warm = EditSession::new(project).unwrap();
    for n in 0..50 {
        edit(&mut warm, n, 1000);
    }
    for block_bytes in [64 * 1024, 1024 * 1024] {
        let mut session = warm.clone();
        let mut previous = session.snapshot().unwrap();
        for n in 0..20 {
            let known: HashSet<[u8; 32]> = previous
                .chunks(block_bytes)
                .map(|chunk| Sha256::digest(chunk).into())
                .collect();
            edit(&mut session, 50 + n, 1000);
            let next = session.snapshot().unwrap();
            if block_bytes == 65536 && n >= 18 {
                if let Some(path) = &output {
                    std::fs::write(path.join(format!("snapshot-{n}.json")), &next).unwrap();
                }
            }
            let began = Instant::now();
            let mut matched = 0;
            let mut blocks = 0;
            for chunk in next.chunks(block_bytes) {
                blocks += 1;
                if known.contains(&<[u8; 32]>::from(Sha256::digest(chunk))) {
                    matched += chunk.len();
                }
            }
            println!(
                "{}",
                serde_json::json!({
                    "block_bytes":block_bytes,"edit":n,"snapshot_bytes":next.len(),
                    "blocks":blocks,"matched_bytes":matched,
                    "potential_reuse_fraction":matched as f64 / next.len() as f64,
                    "hash_lookup_ms":began.elapsed().as_secs_f64()*1000.,
                    "undo_entries":session.history().undo_len(),
                    "scope":"hash matches only; no new codec or persistence"
                })
            );
            previous = next;
        }
        let restored = EditSession::restore(&previous).unwrap();
        assert_eq!(restored.history().project(), session.history().project());
        assert_eq!(restored.history().undo_len(), session.history().undo_len());
        if block_bytes == 65536 {
            if let Some(path) = &output {
                let title = format!("{} 文字数が増える編集", session.history().project().title);
                session
                    .execute(EditRequest {
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        expected_revision: session.history().revision(),
                        action: EditAction::Apply {
                            commands: vec![Command::RenameProject { title }],
                        },
                    })
                    .unwrap();
                let raw = session.snapshot().unwrap();
                assert_eq!(
                    EditSession::restore(&raw).unwrap().history().project(),
                    session.history().project()
                );
                std::fs::write(path.join("snapshot-text-growth.json"), raw).unwrap();
            }
        }
    }
}
