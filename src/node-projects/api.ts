import { invoke, listen } from '../bridge';
import { isNodeRunStatus } from './types';
import type {
  InspectNodeProjectRequest, NodeProjectInspection, NodeProjectsChanged,
  NodeProjectsSnapshot, RemoveNodeProjectRequest, SaveNodeProjectRequest,
  SaveNodeProjectResponse, PrepareNodeRuntimeRequest, NodeRuntimePreparation,
  StartNodeProjectRequest, StopNodeProjectRequest, RestartNodeProjectRequest, NodeRunStatus, NodeRunsSnapshot, ReadNodeLogsRequest, NodeLogBatch,
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

// Calling these wrappers is an explicit execution action; importing a project
// and reading its status never start it. The caller must supply confirmed: true.
export function startNodeProject(request: StartNodeProjectRequest): Promise<NodeRunStatus> {
  return invoke('node_project_start', { request });
}
export function stopNodeProject(request: StopNodeProjectRequest): Promise<NodeRunStatus> {
  return invoke('node_project_stop', { request });
}
export function restartNodeProject(request: RestartNodeProjectRequest): Promise<NodeRunStatus> {
  return invoke('node_project_restart', { request });
}
export function nodeProjectRuns(): Promise<NodeRunsSnapshot> {
  return invoke('node_project_runs');
}
export function readNodeProjectLogs(request: ReadNodeLogsRequest): Promise<NodeLogBatch> {
  return invoke('node_project_logs', { request });
}
export function onNodeProjectRunStatus(callback: (status: NodeRunStatus) => void): Promise<() => void> {
  return listen('node-project-run-status', payload => { if (isNodeRunStatus(payload)) callback(payload); });
}
