import type { LogEntry, ServiceStatus } from './types.ts';

// Capture the whole bounded snapshot, including rows hidden by display filters.
// The text matters for service status and stack traces updated under a stable ID.
export function logViewBaseline(entries: readonly LogEntry[]): ReadonlyMap<string, string> {
  return new Map(entries.map(entry => [entry.id, entry.text]));
}

export function entriesSinceClear(entries: readonly LogEntry[], baseline: ReadonlyMap<string, string> | null): readonly LogEntry[] {
  return baseline ? entries.filter(entry => baseline.get(entry.id) !== entry.text) : entries;
}

// File history keeps its baseline. A service message is only its current state:
// once it changes or recovers, an observed recurrence is a new failure.
export function updateClearedServices(baseline: ReadonlyMap<string, string> | null, services: readonly ServiceStatus[]): ReadonlyMap<string, string> | null {
  if (!baseline) return null;
  const current = logViewBaseline(serviceLogEntries(services));
  let next: Map<string, string> | null = null;
  for (const [id, text] of baseline) {
    if (id.startsWith('current:') && current.get(id) !== text) {
      next ??= new Map(baseline);
      next.delete(id);
    }
  }
  return next ?? baseline;
}

export function serviceLogEntries(services: readonly ServiceStatus[]): LogEntry[] {
  return services.filter(service => service.message).map(service => ({
    id: `current:${service.id}`, source: service.id === 'phpmyadmin' ? 'nginx' : service.id,
    file: 'Current service status', severity: 'error', time: null,
    text: service.message!.length > 4000 ? `${service.message!.slice(0, 4000)}\n… Message shortened.` : service.message!,
  }));
}
