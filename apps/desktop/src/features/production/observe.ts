import { buildActive, type BuildJobsController } from './jobs.svelte';
import type { BuildVideoJob } from '@sungeki/api-client';

export const shouldObserveBuild = (job: BuildVideoJob | null): boolean =>
  !!job && (buildActive(job) || !!job.output?.review.asset_fingerprint);

/** Observe the existing job after transient read failures; never resubmit work. */
export function observeBuildJob(jobs: BuildJobsController): () => void {
  const timer = setInterval(() => {
    if (shouldObserveBuild(jobs.selected) && !jobs.busy && !jobs.pending && !jobs.recoveryError) {
      void jobs.refresh();
    }
  }, 1500);
  return () => clearInterval(timer);
}
