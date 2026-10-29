import { invoke } from '../bridge';
import type { CreationPlanReference, NodeCreationPlan, PlanCreationRequest, TemplateCatalogSnapshot } from './types';
export function nodeProjectTemplates(): Promise<TemplateCatalogSnapshot> {
  return invoke('node_project_templates');
}
export function planNodeProjectCreation(request: PlanCreationRequest): Promise<NodeCreationPlan> {
  return invoke('node_project_creation_plan', { request });
}
// Rechecks an immutable native plan; never consumes it or authorizes execution.
export function validateNodeCreationPlan(request: CreationPlanReference): Promise<NodeCreationPlan> {
  return invoke('node_project_creation_validate_plan', { request });
}

export function generateNodeProject(request: import('./types').GenerateProjectRequest): Promise<import('./types').CreationJobStatus> {
  return invoke('node_project_creation_generate', { request });
}
export function nodeProjectCreationJobs(): Promise<import('./types').CreationJobsSnapshot> {
  return invoke('node_project_creation_jobs');
}
export function cancelNodeProjectCreation(request: { jobId: string }): Promise<void> {
  return invoke('node_project_creation_cancel', { request });
}
export function recoverNodeProjectCreation(request: import('./types').RecoverCreationRequest): Promise<import('./types').CreationJobStatus> {
  return invoke('node_project_creation_recover', { request });
}

export function startNodeProjectInstallation(request: import('./types').InstallRequest): Promise<import('./types').InstallStatus> {
  return invoke('node_project_install_start', { request });
}
export function nodeProjectInstallationStatus(request: { jobId: string }): Promise<import('./types').InstallStatus> {
  return invoke('node_project_install_status', { request });
}
export function nodeProjectInstallationLogs(request: { jobId: string; afterCursor: number }): Promise<import('../node-projects/types').NodeLogBatch> {
  return invoke('node_project_install_logs', { request });
}
export function cancelNodeProjectInstallation(request: import('./types').InstallReference): Promise<import('./types').InstallStatus> {
  return invoke('node_project_install_cancel', { request });
}
export function recoverNodeProjectInstallation(request: import('./types').InstallRecoveryRequest): Promise<import('./types').InstallStatus> {
  return invoke('node_project_install_recover', { request });
}
