// Stage 1 native contracts. No connection credentials or service-control commands.
export type PostgresTool = 'postgres' | 'initdb' | 'pg_ctl' | 'psql' | 'pg_isready' | 'pg_dump' | 'pg_restore' | 'pg_dumpall';
export type PostgresVersion = { major: number; minor: number; patch: number | null };
export type PostgresToolIssue = 'missing' | 'unsafeFile' | 'probeFailed' | 'invalidVersion' | 'deadlineExceeded';
export type PostgresToolsetCheck = 'notChecked' | 'missing' | 'invalid' | 'mixedVersions' | 'verified';
export type PostgresRuntimeCandidate = {
  id: string;
  source: 'fairwayRuntime' | 'environment' | 'homebrew' | 'system' | 'postgresApp' | 'path';
  binDirectory: string;
  tools: { name: PostgresTool; path: string | null; version: PostgresVersion | null; issue: PostgresToolIssue | null }[];
  // These describe --version checks only; they do not indicate a running/ready/owned server.
  serverTools: PostgresToolsetCheck;
  clientTools: PostgresToolsetCheck;
};
export type PostgresRuntimeInventory = { candidates: PostgresRuntimeCandidate[]; truncated: boolean; notes: string[] };
export type InspectPostgresRuntimeRequest = { candidateId: string; confirmed: true };
export type PostgresError = { code: 'busy' | 'confirmationRequired' | 'staleCandidate' | 'unavailable'; message: string };
export function isPostgresError(value: unknown): value is PostgresError {
  if (!value || typeof value !== 'object') return false;
  const error = value as Record<string, unknown>;
  return typeof error.message === 'string' && typeof error.code === 'string'
    && ['busy', 'confirmationRequired', 'staleCandidate', 'unavailable'].includes(error.code);
}
