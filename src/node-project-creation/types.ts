// Reviewed local generation only. No dependency installer or project-execution API.
export const creationErrorCodes = [
  'unsupportedPlatform', 'unavailable', 'invalidName', 'invalidDestination', 'destinationExists',
  'invalidTemplate', 'unsupportedPackageManager', 'runtimeUnavailable', 'runtimeChanged',
  'npmUnavailable', 'requirementConflict', 'environmentUnavailable', 'runtimeValidationFailed',
  'catalogChanged', 'invalidRequest', 'duplicateRequest', 'planUnavailable', 'planTampered',
  'planExpired', 'planStale', 'planLimit', 'planConsumed', 'confirmationRequired', 'generationBusy',
  'unsafeTemplate', 'filesystemFailed', 'journalUnavailable', 'staleJob', 'jobUnavailable', 'recoveryUnsafe',
  'cancellationTooLate', 'generationCancelled', 'registrationFailed',
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
  qualification: 'localGeneration'; lockfileStatus: 'notPrepared'; intendedFiles: string[];
  scripts: Record<string, string>; dependencies: TemplateDependency[];
  provenance: { kind: 'fairwayAuthoredTemplate'; referenceUrl: string; upstreamRevision: string | null; license: string; note: string };
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
  files: Record<string, { bytes: number; sha256: string }>;
  warnings: string[];
}

export interface GenerateProjectRequest extends CreationPlanReference { confirmed: true }
export interface CreationJobReference { jobId: string; expectedRevision: number }
export type CreationPhase = 'prepared' | 'staging' | 'ready' | 'publishing' | 'needsRegistration' | 'completed' | 'cancelled' | 'preserved';
export interface CreationJobStatus {
  jobId: string; revision: number; phase: CreationPhase; destination: string; stagingPath: string;
  projectId: string | null; error: CreationError | null; active: boolean; recoveryRequired: boolean;
}
export interface CreationJobsSnapshot { supported: boolean; jobs: CreationJobStatus[]; errors: string[] }
export interface RecoverCreationRequest extends CreationJobReference { action: 'discardStaging' | 'retryRegistration'; confirmed: true }
