// Catalog-only foundation. No project creation or installation API exists yet.
export const creationErrorCodes = [
  'unsupportedPlatform', 'unavailable', 'invalidName', 'invalidDestination', 'destinationExists',
  'invalidTemplate', 'unsupportedPackageManager', 'runtimeUnavailable', 'runtimeChanged',
  'npmUnavailable', 'requirementConflict', 'environmentUnavailable', 'runtimeValidationFailed',
  'catalogChanged', 'invalidRequest', 'duplicateRequest', 'planUnavailable', 'planTampered',
  'planExpired', 'planStale', 'planLimit',
] as const;
export type CreationErrorCode = typeof creationErrorCodes[number];
export interface CreationError { code: CreationErrorCode; message: string }
export function isCreationError(value: unknown): value is CreationError {
  if (!value || typeof value !== 'object') return false;
  const error = value as Partial<CreationError>;
  return typeof error.message === 'string' && creationErrorCodes.includes(error.code as CreationErrorCode);
}
export interface TemplateDependency {
  name: string; version: string; development: boolean; nodeRange: string | null;
  license: string; metadataUrl: string;
}
export interface NodeTemplate {
  id: string; revision: string; title: string; language: 'javascript' | 'typescript';
  nodeRange: string; packageManager: { name: 'npm'; versionRange: string };
  qualification: 'planningOnly'; lockfileStatus: 'notPrepared'; intendedFiles: string[];
  scripts: Record<string, string>; dependencies: TemplateDependency[];
  provenance: { kind: 'fairwayAuthoredDesign'; referenceUrl: string; upstreamRevision: string | null; license: string; note: string };
}
export interface TemplateCatalogSnapshot {
  supported: boolean; catalogRevision: string;
  catalog: { schemaVersion: 1; revision: string; templates: NodeTemplate[] };
  runtimes: { id: string; nodePath: string; version: string; source: string }[];
  runtimeError: string | null; discoveryNote: string;
}
export interface PlanCreationRequest {
  requestId: string; catalogRevision: string; templateId: string; projectName: string;
  parent: string; runtimeId: string; packageManager: 'npm';
}
export interface CreationPlanReference { planId: string; digest: string }
export interface NodeCreationPlan {
  schemaVersion: 1; id: string; digest: string; catalogRevision: string;
  createdAt: number; expiresAt: number; projectName: string; parent: string; destination: string;
  template: NodeTemplate;
  tools: { runtimeId: string; nodePath: string; nodeVersion: string; npmCliPath: string; npmVersion: string; revision: string };
  installation: { packageManager: 'npm'; intendedCommand: 'ci'; ignoreScripts: true;
    registry: string; cacheScope: 'appOwned'; dependencyLock: 'notPrepared'; installationAvailable: false };
  warnings: string[];
}
