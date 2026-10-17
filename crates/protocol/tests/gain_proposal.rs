use sungeki_protocol::{ProposeSpeechGainArgs, SessionRequest};
#[test]
fn gain_proposal_requires_minor_39() {
    let request = SessionRequest::ProposeSpeechGain {
        args: ProposeSpeechGainArgs {
            session_id: "s".into(),
            expected_revision: 0,
            job_id: "j".into(),
            clip_id: "c".into(),
            target_lufs: -22.,
            max_true_peak_db: -1.,
        },
    };
    assert_eq!(request.minimum_protocol_minor(), 39);
}
