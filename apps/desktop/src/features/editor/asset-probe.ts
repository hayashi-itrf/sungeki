import { open } from '@tauri-apps/plugin-dialog';
import type { AssetProbe } from '@sungeki/api-client';
import { hostApi } from '../../shared/host-api';

export async function probeAsset(path: string, registered: AssetProbe): Promise<AssetProbe> {
  switch (registered.type) {
    case 'image': {
      const result = await hostApi.session({ type: 'probe_image', path });
      if (result.type === 'image_probed') return { type: 'image', info: result.probe };
      break;
    }
    case 'audio': {
      const result = await hostApi.session({ type: 'probe_audio', path });
      if (result.type === 'audio_probed') return { type: 'audio', info: result.probe };
      break;
    }
    case 'video': {
      const result = await hostApi.session({ type: 'probe_video', path });
      if (result.type === 'video_probed') return { type: 'video', info: result.probe };
      break;
    }
    case 'psd': {
      const result = await hostApi.session({ type: 'probe_psd', path });
      if (result.type === 'psd_probed') return { type: 'psd', info: result.probe };
      break;
    }
    case 'vrm': {
      const result = await hostApi.session({ type: 'probe_vrm', path, kind: registered.info.inspection.kind });
      if (result.type === 'vrm_probed') return { type: 'vrm', info: result.probe };
      break;
    }
  }
  throw new Error('素材の検査結果を取得できませんでした。');
}
export async function chooseReplacement(probe: AssetProbe) {
  const extensions = {
    image: ['png', 'jpg', 'jpeg', 'webp'], audio: ['wav', 'mp3'],
    video: ['mp4', 'mov', 'mkv', 'webm'], psd: ['psd'],
    vrm: probe.type === 'vrm' && probe.info.inspection.kind === 'motion' ? ['vrma'] : ['vrm'],
  }[probe.type];
  const path = await open({ title: '素材を再指定', multiple: false, directory: false,
    filters: [{ name: '同じ種類の素材', extensions }] });
  return path ? { path, probe: await probeAsset(path, probe) } : null;
}
