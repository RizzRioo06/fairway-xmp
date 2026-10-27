import { invoke, listen } from '../bridge';
import type {
  InspectNodeProjectRequest, NodeProjectInspection, NodeProjectsChanged,
  NodeProjectsSnapshot, RemoveNodeProjectRequest, SaveNodeProjectRequest,
  SaveNodeProjectResponse, PrepareNodeRuntimeRequest, NodeRuntimePreparation,
} from './types';

export function nodeProjectsSnapshot(): Promise<NodeProjectsSnapshot> {
  return invoke('node_projects_snapshot');
}

export function inspectNodeProject(request: InspectNodeProjectRequest): Promise<NodeProjectInspection> {
  return invoke('node_project_inspect', { request });
}

export function saveNodeProject(request: SaveNodeProjectRequest): Promise<SaveNodeProjectResponse> {
  return invoke('node_project_save', { request });
}

export function removeNodeProject(request: RemoveNodeProjectRequest): Promise<NodeProjectsChanged> {
  return invoke('node_project_remove', { request });
}

// Subscribe before the initial snapshot; events invalidate the registry view.
export function onNodeProjectsChanged(callback: (event: NodeProjectsChanged) => void): Promise<() => void> {
  return listen('node-projects-changed', payload => {
    if (payload && typeof payload === 'object' && 'revision' in payload && typeof payload.revision === 'string') {
      callback({ revision: payload.revision });
    }
  });
}

export function prepareNodeRuntime(request: PrepareNodeRuntimeRequest): Promise<NodeRuntimePreparation> {
  return invoke('node_project_prepare_runtime', { request });
}
