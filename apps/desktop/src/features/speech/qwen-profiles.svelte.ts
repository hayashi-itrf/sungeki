import { locale } from '../../shared/i18n/locale.svelte';
import { missingConfigurationPath } from './profile-configuration-error';
import type { HostApi, QwenProfileSummary } from '@sungeki/api-client';

export class QwenProfiles {
  profiles = $state<QwenProfileSummary[]>([]);
  selectedId = $state('');
  busy = $state(false);
  loaded = $state(false);
  error = $state('');
  configurationPath = $state<string | null>(null);
  private alive = true;
  constructor(private api: Pick<HostApi, 'session'>) {}
  get ready() { return this.loaded && !this.busy && this.profiles.some(p => p.id === this.selectedId); }
  dispose() { this.alive = false; }
  async refresh() {
    if (!this.alive || this.busy) return;
    this.busy = true; this.error = ''; this.configurationPath = null;
    try {
      const response = await this.api.session({ type: 'speech_job', request: { type: 'list_qwen_profiles' } });
      if (!this.alive) return;
      if (response.type !== 'qwen_profiles' || !Array.isArray(response.profiles) || response.profiles.length > 32
        || response.profiles.some(p => !p || typeof p.id !== 'string' || typeof p.name !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(p.id) || !p.name.trim())
        || new Set(response.profiles.map(p => p.id)).size !== response.profiles.length) throw new Error(locale.text('speechProfilesResponseInvalid'));
      this.profiles = response.profiles;
      if (!this.profiles.some(p => p.id === this.selectedId)) this.selectedId = !this.loaded && !this.selectedId && this.profiles.length === 1 ? this.profiles[0].id : '';
      this.loaded = true;
    } catch (error) {
      if (!this.alive) return;
      this.profiles = []; this.selectedId = ''; this.loaded = false;
      this.configurationPath = missingConfigurationPath(error);
      this.error = error && typeof error === 'object' && 'message' in error ? String(error.message) : String(error);
    } finally { if (this.alive) this.busy = false; }
  }
}
