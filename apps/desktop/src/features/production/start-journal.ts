import { extraText } from '../../shared/i18n/extra-ui';
import type { StartBuildVideoArgs } from '@sungeki/api-client';
export interface StartJournal {
  read(): StartBuildVideoArgs | null;
  write(args: StartBuildVideoArgs): void;
  verify(args: StartBuildVideoArgs): void;
  clear(args: StartBuildVideoArgs): void;
}
const key = 'sungeki.pending-build-start.v1';
type StoragePort = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
export class BrowserStartJournal implements StartJournal {
  constructor(private storage: () => StoragePort = () => window.localStorage) {}
  read(): StartBuildVideoArgs | null {
    const raw = this.storage().getItem(key);
    if (raw === null) return null;
    let record;
    try { record = JSON.parse(raw); } catch { throw new Error(extraText("制作開始の保存記録を読み取れません。記録は削除していません。")); }
    const args = record?.args;
    if (record?.version !== 1 || !args || typeof args.job_id !== 'string' || !/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(args.job_id)
      || typeof args.project_path !== 'string' || !args.project_path.length
      || !['base_url', 'engine_version'].every(k => typeof args[k] === 'string')
      || !args.recipe || typeof args.recipe !== 'object' || typeof args.recipe.title !== 'string' || !Array.isArray(args.recipe.scenes)) {
      throw new Error(extraText("制作開始の保存記録が不正です。新しい制作は開始せず、記録を保持します。"));
    }
    // Managed-only recipes intentionally have no VOICEVOX URL/version.
    // Recipe acceptance belongs to the host. Restoration never dispatches it.
    return args as StartBuildVideoArgs;
  }
  write(args: StartBuildVideoArgs) {
    const old = this.read();
    if (old && JSON.stringify(old) !== JSON.stringify(args)) throw new Error(extraText("未確認の制作開始記録が残っています。状態を再確認してください。"));
    this.storage().setItem(key, JSON.stringify({ version: 1, args }));
  }
  verify(args: StartBuildVideoArgs) {
    const old = this.read();
    if (!old || JSON.stringify(old) !== JSON.stringify(args)) throw new Error(extraText("保存された制作開始要求が画面と一致しません。再送せず、状態の確認を行ってください。"));
  }
  clear(args: StartBuildVideoArgs) {
    const old = this.read();
    if (old && JSON.stringify(old) !== JSON.stringify(args)) throw new Error(extraText("別の制作開始記録があるため保持しました。"));
    if (old) this.storage().removeItem(key);
  }
}
