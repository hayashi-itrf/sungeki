import { expect, test } from 'vitest';
import type { LoudnessJob, SessionSnapshot, SpeechGainProposal } from '@sungeki/api-client';
import { proposalCurrent } from './proposal';
function fixture() {
  const snapshot = { session_id: 's', revision: 3, project: { clips: [{ id: 'c', content: { type: 'speech', gain_db: -3, audio: { asset_id: 'a' } } }] } } as SessionSnapshot;
  const job = { session_id: 's', job_id: 'j', source_revision: 3, phase: 'succeeded', stale: false, asset_id: 'a', output: { source_sha256: 'hash' } } as LoudnessJob;
  const proposal = { session_id: 's', source_revision: 3, job_id: 'j', clip_id: 'c', asset_id: 'a', source_sha256: 'hash', current_gain_db: -3,
    estimate: { target_lufs: -22, max_true_peak_db: -1, gain_db: 2.5 }, command: { type: 'set_speech_gain', clip_id: 'c', gain_db: 2.5 } } as SpeechGainProposal;
  return { snapshot, job, proposal };
}
test('only the reviewed source-relative command can be applied to the matching snapshot', () => {
  const { snapshot, job, proposal } = fixture();
  expect(proposalCurrent(proposal, snapshot, job, 'c', -22, -1)).toBe(true);
  expect(proposal.command).toEqual({ type: 'set_speech_gain', clip_id: 'c', gain_db: 2.5 });
});
test('changed session revision selection targets and source reject stale proposals', () => {
  const { snapshot, job, proposal } = fixture();
  for (const changed of [{ ...snapshot, revision: 4 }, { ...snapshot, session_id: 'other' }])
    expect(proposalCurrent(proposal, changed, job, 'c', -22, -1)).toBe(false);
  expect(proposalCurrent(proposal, snapshot, job, 'other', -22, -1)).toBe(false);
  expect(proposalCurrent(proposal, snapshot, job, 'c', -16, -1)).toBe(false);
  expect(proposalCurrent(proposal, snapshot, job, 'c', -22, undefined)).toBe(false);
  for (const changed of [{ ...job, stale: true }, { ...job, asset_id: 'other' }, { ...job, job_id: 'other' }])
    expect(proposalCurrent(proposal, snapshot, changed, 'c', -22, -1)).toBe(false);
  expect(proposalCurrent({ ...proposal, source_sha256: 'other' }, snapshot, job, 'c', -22, -1)).toBe(false);
});
test('mismatched command, non-finite gain and changed current gain cannot be applied', () => {
  const { snapshot, job, proposal } = fixture();
  const valid = (p: SpeechGainProposal) => proposalCurrent(p, snapshot, job, 'c', -22, -1);
  expect(valid({ ...proposal, command: { type: 'set_speech_gain', clip_id: 'other', gain_db: 2.5 } })).toBe(false);
  expect(valid({ ...proposal, command: { type: 'set_speech_gain', clip_id: 'c', gain_db: 5.5 } })).toBe(false);
  expect(valid({ ...proposal, current_gain_db: null })).toBe(false);
  expect(valid({ ...proposal, estimate: { ...proposal.estimate, gain_db: NaN } })).toBe(false);
});
