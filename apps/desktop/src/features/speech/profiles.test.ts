import { expect, test, vi } from 'vitest';
import { render } from 'svelte/server';
import { Sbv2Profiles } from './profiles.svelte';
import Sbv2ProfilePicker from './Sbv2ProfilePicker.svelte';
const profile = { id: 'local-jp', name: '日本語の音声', cpu_only: true };
test('loads named profiles without engine discovery and selects a single entry', async () => {
  const session = vi.fn().mockResolvedValue({ type: 'sbv2_profiles', profiles: [profile] });
  const profiles = new Sbv2Profiles({ session });
  await profiles.refresh();
  expect(session).toHaveBeenCalledExactlyOnceWith({ type: 'speech_job', request: { type: 'list_sbv2_profiles' } });
  expect(profiles.selectedId).toBe('local-jp'); expect(profiles.ready).toBe(true);
  const html = render(Sbv2ProfilePicker, { props: { profiles } }).body;
  expect(html).toContain('日本語の音声（CPU）');
  expect(html).toContain('一覧の取得ではエンジンを起動しません');
  expect(html).not.toContain('type="text"');
  expect(html).toContain('<button type="button"');
});
test('multiple profiles require a choice and refresh preserves only existing choices', async () => {
  const second = { ...profile, id: 'second', name: '別の音声' };
  const session = vi.fn().mockResolvedValue({ type: 'sbv2_profiles', profiles: [profile, second] });
  const profiles = new Sbv2Profiles({ session });
  await profiles.refresh(); expect(profiles.selectedId).toBe(''); expect(profiles.ready).toBe(false);
  profiles.selectedId = 'second'; await profiles.refresh(); expect(profiles.selectedId).toBe('second');
  session.mockResolvedValue({ type: 'sbv2_profiles', profiles: [profile] });
  await profiles.refresh(); expect(profiles.selectedId).toBe(''); expect(profiles.ready).toBe(false);
  session.mockResolvedValue({ type: 'sbv2_profiles', profiles: [] });
  await profiles.refresh(); expect(profiles.selectedId).toBe(''); expect(profiles.ready).toBe(false);
  expect(render(Sbv2ProfilePicker, { props: { profiles } }).body).toContain('登録済みの起動設定はありません');
});
test('failed refresh clears stale choices and presents the host explanation', async () => {
  const session = vi.fn().mockResolvedValueOnce({ type: 'sbv2_profiles', profiles: [profile] })
    .mockRejectedValueOnce({ message: 'Sungeki用SBV2起動設定がありません' });
  const profiles = new Sbv2Profiles({ session });
  await profiles.refresh(); await profiles.refresh();
  expect(profiles.ready).toBe(false); expect(profiles.profiles).toEqual([]);
  expect(render(Sbv2ProfilePicker, { props: { profiles } }).body).toContain('Sungeki用SBV2起動設定がありません');
});
test('invalid duplicate IDs and late disposed responses cannot enable start', async () => {
  const profiles = new Sbv2Profiles({ session: vi.fn().mockResolvedValue({ type: 'sbv2_profiles', profiles: [profile, profile] }) });
  await profiles.refresh(); expect(profiles.ready).toBe(false); expect(profiles.error).not.toBe('');
  let resolve!: (response: any) => void;
  const session = vi.fn(() => new Promise<any>(r => { resolve = r; }));
  const disposed = new Sbv2Profiles({ session });
  const waiting = disposed.refresh(); await disposed.refresh();
  expect(session).toHaveBeenCalledTimes(1);
  disposed.dispose(); resolve({ type: 'sbv2_profiles', profiles: [profile] }); await waiting;
  expect(disposed.profiles).toEqual([]); expect(disposed.selectedId).toBe('');
});
