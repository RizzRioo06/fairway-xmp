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
