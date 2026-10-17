//! Shared source-sample selection for original audio and aligned video audio.
use sungeki_project::TimeUs;
pub(crate) fn range(start: TimeUs, duration: TimeUs, rate: u32, frames: u64) -> (u64, u64) {
    let at = |time: u128| (time * u128::from(rate)).div_ceil(1_000_000);
    (
        at(u128::from(start.0)) as u64,
        at(u128::from(start.0) + u128::from(duration.0)).min(u128::from(frames)) as u64,
    )
}
pub(crate) fn trim_loop(first: u64, end: u64, looped: bool) -> String {
    let mut filter = format!(
        "atrim=start_sample={first}:end_sample={end},asetpts=PTS-STARTPTS,aformat=sample_fmts=dbl,"
    );
    if looped {
        filter.push_str(&format!("aloop=loop=-1:size={}:start=0,", end - first));
    }
    filter
}
