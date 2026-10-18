import { expect, test, vi } from 'vitest';
import type { LocalSpeechEngine, SessionSnapshot, SessionResponse } from '@sungeki/api-client';
import { SpeechJobsController } from './jobs.svelte';
function snapshot(engine: LocalSpeechEngine): SessionSnapshot {
  const common = { speaker_uuid: '3c37646f-3881-5374-2a83-149267990abc', style_id: engine === 'aivis_speech' ? -12 : 0, speed_scale: 1, pitch_scale: 0, volume_scale: 1, pre_phoneme_length: .1, post_phoneme_length: .1 };
  return { session_id: 's', revision: 7, project: {
    clips: [{ id: 'clip', content: { type: 'speech', character_id: 'char' } }],
    characters: [{ id: 'char', voice: { type: 'local_http', settings: engine === 'aivis_speech' ? { engine, ...common, emotion_scale: 1, tempo_dynamics_scale: 1 } : { engine, ...common, intonation_scale: 1 } } }],
  } } as SessionSnapshot;
}
for (const engine of ['aivis_speech', 'coeiroink'] as const) {
  test(`${engine} dispatch and uncertain retry preserve the selected engine and job`, async () => {
    const input = snapshot(engine);
    const voice = input.project.characters[0]!.voice;
    if (voice.type !== 'local_http') throw new Error('fixture');
    const catalog: SessionResponse = { type: 'local_speech_catalog', catalog: { engine, version: 'v1@catalog', speakers: [{ speaker_uuid: voice.settings.speaker_uuid, name: 'voice', styles: [{ id: voice.settings.style_id, name: 'normal' }] }] } };
    const session = vi.fn().mockResolvedValueOnce(catalog).mockRejectedValueOnce({ code: 'HOST_UNAVAILABLE', message: 'uncertain' });
    const jobs = new SpeechJobsController({ session }, 's');
    await jobs.start(input, 'clip', 'http://127.0.0.1:50021');
    expect(session.mock.calls[0]![0].type).toBe('get_local_speech_catalog');
    expect(session.mock.calls[1]![0].request.type).toBe('start_local');
    const pending = jobs.pending;
    expect(pending).not.toBeNull();
    session.mockResolvedValueOnce({ type: 'speech_job', job: { session_id: 's', job_id: pending!.job_id, clip_id: 'clip', source_revision: 7, phase: 'queued' } });
    await jobs.retry();
    expect(session.mock.calls[2]![0].request).toEqual({ type: 'start_local', args: pending });
    expect(jobs.pending).toBeNull();
  });
}
