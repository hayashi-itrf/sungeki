import type { LoudnessJob, SessionSnapshot, SpeechGainProposal } from '@sungeki/api-client';
export function proposalCurrent(p: SpeechGainProposal, s: SessionSnapshot, job: LoudnessJob, clipId: string, target: number | undefined, peak: number | undefined): boolean {
  const clip = s.project.clips.find(c => c.id === clipId);
  return s.session_id === p.session_id && s.revision === p.source_revision
    && job.session_id === s.session_id && job.job_id === p.job_id && job.phase === 'succeeded'
    && !job.stale && job.source_revision === s.revision && job.asset_id === p.asset_id
    && job.output?.source_sha256 === p.source_sha256
    && clip?.content.type === 'speech' && clip.id === p.clip_id
    && clip.content.audio?.asset_id === p.asset_id && (clip.content.gain_db ?? null) === p.current_gain_db
    && p.estimate.target_lufs === target && p.estimate.max_true_peak_db === peak
    && Number.isFinite(p.estimate.gain_db) && p.estimate.gain_db >= -60 && p.estimate.gain_db <= 24
    && p.command.type === 'set_speech_gain' && p.command.clip_id === p.clip_id
    && p.command.gain_db === p.estimate.gain_db;
}
