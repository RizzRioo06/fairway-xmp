import type { LogEntry, ServiceStatus } from './types.ts';

// Capture the whole bounded snapshot, including rows hidden by display filters.
// The text matters for service status and stack traces updated under a stable ID.
export function logViewBaseline(entries: readonly LogEntry[]): ReadonlyMap<string, string> {
  return new Map(entries.map(entry => [entry.id, entry.text]));
}

export function entriesSinceClear(entries: readonly LogEntry[], baseline: ReadonlyMap<string, string> | null): readonly LogEntry[] {
  return baseline ? entries.filter(entry => baseline.get(entry.id) !== entry.text) : entries;
}

export function serviceLogEntries(services: readonly ServiceStatus[]): LogEntry[] {
  return services.filter(service => service.message).map(service => ({
    id: `current:${service.id}:${service.messageGeneration ?? 0}`, source: service.id === 'phpmyadmin' ? 'nginx' : service.id,
    file: 'Current service status', severity: 'error', time: null,
    text: service.message!.length > 4000 ? `${service.message!.slice(0, 4000)}\n… Message shortened.` : service.message!,
  }));
}
