import { useCallback, useEffect, useRef, useState } from 'react';
import { canControl } from '../bridge';
import * as api from './api';
import { complete, creationError, mergeJob } from './model';
import type { CreationJobStatus, CreationJobsSnapshot } from './types';
const selectionKey = 'fairway.nodeCreation.selection';
export function rememberedCreation(): string | null { try { const id = sessionStorage.getItem(selectionKey); return id && /^[a-f0-9]{32}$/i.test(id) ? id : null; } catch { return null; } }
export function rememberCreation(id: string | null) { try { if (id) sessionStorage.setItem(selectionKey, id); else sessionStorage.removeItem(selectionKey); } catch { /* The native journal remains authoritative. */ } }
export function useCreationJobs(active: boolean, onRegistered: () => Promise<void>) {
  const [jobs, setJobs] = useState<CreationJobStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<{ jobId: string; text: string } | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const [pending, setPending] = useState(false);
  const action = useRef(false);
  const epoch = useRef(0);
  const seen = useRef(new Set<string>());
  const reload = useRef<() => Promise<CreationJobsSnapshot | null>>(async () => null);
  const refresh = useCallback(() => { epoch.current++; setActionError(null); return reload.current(); }, []);
  useEffect(() => {
    if (!active || !canControl) return;
    let disposed = false, timer = 0;
    let flight: Promise<CreationJobsSnapshot | null> | null = null;
    const load = (): Promise<CreationJobsSnapshot | null> => {
      if (flight) return flight;
      const observed = epoch.current;
      flight = (async () => {
        try {
          const snapshot = await api.nodeProjectCreationJobs();
          if (disposed || observed !== epoch.current) return null;
          setJobs(previous => snapshot.jobs.map(job => mergeJob(previous.find(old => old.jobId === job.jobId), job)));
          setError(null); setErrors(snapshot.errors);
          const completed = snapshot.jobs.filter(complete);
          const changed = completed.some(job => !seen.current.has(job.jobId));
          completed.forEach(job => seen.current.add(job.jobId));
          if (changed) await onRegistered();
          return snapshot;
        } catch (error) { if (!disposed) setError(creationError(error)); return null; }
      })().finally(() => { flight = null; });
      return flight;
    };
    reload.current = async () => { if (flight) await flight; return disposed ? null : load(); };
    async function poll() {
      const value = await load();
      if (!disposed) timer = window.setTimeout(() => { void poll(); }, value?.jobs.some(job => job.active) ? 500 : 1800);
    }
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); reload.current = async () => null; /* Never cancel an owned native job on view teardown. */ };
  }, [active, onRegistered]);
  async function perform(job: CreationJobStatus, kind: 'cancel' | 'recover') {
    if (action.current) return;
    action.current = true; epoch.current++; setPending(true); setActionError(null);
    try {
      if (kind === 'cancel' && job.installation) await api.cancelNodeProjectInstallation({ jobId: job.installation.jobId, expectedRevision: job.installation.revision });
      else if (job.installation) await api.recoverNodeProjectInstallation({ jobId: job.installation.jobId, expectedRevision: job.installation.revision, action: 'finalize', confirmed: true });
      else if (kind === 'recover') await api.recoverNodeProjectCreation({ jobId: job.jobId, expectedRevision: job.revision, action: 'retryRegistration', confirmed: true });
    } catch (error) { setActionError({ jobId: job.jobId, text: creationError(error) }); }
    finally { action.current = false; epoch.current++; setPending(false); await reload.current(); }
  }
  return { jobs, error, actionError, errors, pending, refresh, perform };
}
