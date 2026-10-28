import { useCallback, useEffect, useRef, useState } from 'react';
import { canControl } from '../bridge';
import * as api from './api';
import { mergeRun, nodeError } from './model';
import type { NodeProjectsSnapshot, NodeRunStatus } from './types';

export function useProjects(active: boolean) {
  const [snapshot, setSnapshot] = useState<NodeProjectsSnapshot | null>(null);
  const [runs, setRuns] = useState<Record<string, NodeRunStatus>>({});
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connection, setConnection] = useState(0);
  const reload = useRef<() => Promise<void>>(async () => {});
  const refresh = useCallback(() => reload.current(), []);
  useEffect(() => {
    if (!active || !canControl) return;
    let disposed = false, dirty = false, epoch = 0, subscribed = false;
    let flight: Promise<void> | null = null;
    const unsubscribers: (() => void)[] = [];
    const load = (): Promise<void> => {
      dirty = true;
      if (!subscribed) return Promise.resolve();
      if (flight) return flight;
      flight = (async () => {
        while (dirty && !disposed) {
          dirty = false;
          const observed = epoch;
          try {
            const [registry, status] = await Promise.all([api.nodeProjectsSnapshot(), api.nodeProjectRuns()]);
            if (disposed) return;
            if (observed !== epoch) { dirty = true; continue; }
            setSnapshot(registry);
            setRuns(previous => {
              const next: Record<string, NodeRunStatus> = {};
              for (const run of status.runs) next[run.projectId] = mergeRun(previous[run.projectId], run, true);
              return next;
            });
            setError(null);
          } catch (error) { if (!disposed) setError(nodeError(error)); }
          finally { if (!disposed) setLoading(false); }
        }
      })().finally(() => { flight = null; });
      return flight;
    };
    setLoading(true);
    // Await both subscriptions before fetching. Epochs reject snapshots captured
    // before a newer event, and only one refresh is ever in flight.
    void (async () => {
      try {
        for (const subscribe of [
          () => api.onNodeProjectsChanged(() => { epoch++; void load(); }),
          () => api.onNodeProjectRunStatus(run => {
            epoch++;
            if (!disposed) setRuns(previous => ({ ...previous, [run.projectId]: mergeRun(previous[run.projectId], run) }));
            void load();
          }),
        ]) {
          const off = await subscribe();
          if (disposed) { off(); return; }
          unsubscribers.push(off);
        }
        if (!disposed) { subscribed = true; reload.current = load; await load(); }
      } catch (error) { if (!disposed) { setError(nodeError(error)); setLoading(false); reload.current = async () => setConnection(value => value + 1); } }
    })();
    const visible = () => { if (!document.hidden) void load(); };
    const timer = window.setInterval(visible, 3000);
    document.addEventListener('visibilitychange', visible);
    window.addEventListener('focus', visible);
    return () => {
      disposed = true; reload.current = async () => {};
      window.clearInterval(timer); unsubscribers.forEach(off => off());
      document.removeEventListener('visibilitychange', visible); window.removeEventListener('focus', visible);
      // Unsubscribing a view never stops a native project.
    };
  }, [active, connection]);
  return { snapshot, runs, error, loading, refresh };
}
