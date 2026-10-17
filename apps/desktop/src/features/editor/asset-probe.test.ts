import { expect, test, vi } from 'vitest';
import type { AssetProbe } from '@sungeki/api-client';
const mocks = vi.hoisted(() => ({ open: vi.fn(), session: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }));
vi.mock('../../shared/host-api', () => ({ hostApi: { session: mocks.session } }));
import { chooseReplacement, probeAsset } from './asset-probe';

test('every original is probed by its own host entry, retaining VRM model/motion role', async () => {
  for (const type of ['image', 'audio', 'video', 'psd', 'vrm'] as const) {
    for (const kind of type === 'vrm' ? ['model', 'motion'] as const : ['model'] as const) {
      const registered = { type, info: { inspection: { kind } } } as AssetProbe;
      const info = { sha256: 'verified' };
      mocks.session.mockResolvedValueOnce({ type: `${type}_probed`, probe: info });
      expect(await probeAsset('/original', registered)).toEqual({ type, info });
      expect(mocks.session).toHaveBeenLastCalledWith({ type: `probe_${type}`, path: '/original',
        ...(type === 'vrm' ? { kind } : {}) });
    }
  }
});
test('cancelled chooser does not probe and unexpected responses cannot become replacement metadata', async () => {
  mocks.session.mockClear();
  mocks.open.mockResolvedValueOnce(null);
  const registered = { type: 'audio', info: {} } as AssetProbe;
  expect(await chooseReplacement(registered)).toBeNull();
  expect(mocks.session).not.toHaveBeenCalled();
  mocks.session.mockResolvedValueOnce({ type: 'image_probed', probe: {} });
  await expect(probeAsset('/audio', registered)).rejects.toThrow('検査結果');
});
