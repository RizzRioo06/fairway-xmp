import { invoke } from '../bridge';
import type { InspectPostgresRuntimeRequest, PostgresRuntimeCandidate, PostgresRuntimeInventory } from './types';

// No automatic invocation/startup polling. A later explicit UI can use these foundations.
export function discoverPostgresRuntimes(): Promise<PostgresRuntimeInventory> {
  return invoke('postgres_runtime_discover');
}
export function inspectPostgresRuntime(request: InspectPostgresRuntimeRequest): Promise<PostgresRuntimeCandidate> {
  return invoke('postgres_runtime_inspect', { request });
}
