//! Worker completion and private intermediates are returned separately so a
//! failed/cancelled render can reclaim files after its terminal state is durable.
use crate::render_record::{Input, invalid};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    sync::atomic::AtomicBool,
};
use sungeki_protocol::{ApiError, RenderOutput};
use sungeki_storage::RenderIntermediates;
type Output = Result<(RenderOutput, Vec<String>), ApiError>;

pub(super) fn run(
    input: &Input,
    flag: &AtomicBool,
    progress: &mut dyn FnMut(sungeki_render::RenderProgress),
) -> (Output, Option<RenderIntermediates>) {
    let mut guard = None;
    let mut settled = false;
    let output = (|| {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&input.directory)
            .map_err(invalid)?;
        guard = Some(RenderIntermediates::new(&input.directory).map_err(invalid)?);
        fs::File::open(
            input
                .directory
                .parent()
                .ok_or_else(|| invalid("作業領域の親がありません"))?,
        )
        .and_then(|f| f.sync_all())
        .map_err(invalid)?;
        let (artifact, verification) = super::execution::run(input, flag, &mut settled, progress)?;
        let mut report = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(input.directory.join("verification.json"))
            .map_err(invalid)?;
        report.write_all(&verification).map_err(invalid)?;
        report.sync_all().map_err(invalid)?;
        fs::File::open(&input.directory)
            .and_then(|f| f.sync_all())
            .map_err(invalid)?;
        let chunks = crate::render_api::read::index::create(
            std::path::Path::new(&artifact.path),
            artifact.byte_length,
            &artifact.sha256,
            flag,
        )?;
        Ok((artifact, chunks))
    })();
    let cleanup = if settled {
        guard.and_then(|guard| match guard.capture() {
            Ok(cleanup) => Some(cleanup),
            Err(error) => {
                eprintln!("render intermediates retained: {error}");
                None
            }
        })
    } else {
        None
    };
    (output, cleanup)
}
