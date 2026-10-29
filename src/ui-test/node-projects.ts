// UI-test mode only. No native processes, project files or manager settings.
import type { NodeProjectConfig, NodeProjectInspection, NodeRunStatus, NodeLogRecord, NodeProjectsErrorCode } from '../node-projects/types';
import { activeRun } from '../node-projects/model';
type Emit = (event: string, payload: unknown) => void;
export let options: { root?: string; manager?: string; missingRoot?: boolean; missingRuntime?: boolean; noRuntimes?: boolean; registryError?: boolean; inspectionError?: boolean; logsError?: boolean; truncated?: boolean; staleStop?: boolean; requirementConflict?: boolean } = {};
let projects: NodeProjectConfig[] = [], runs: Record<string, NodeRunStatus> = {};
let history = new Map<string, NodeLogRecord[]>();
let revision = 1, serial = 0;
let event: Emit = () => {};
let generation = 0;
const runtimes = [22, 24].map(major => ({ id: `node-${major}`, nodePath: `/controlled/node${major}/bin/node`, version: `v${major}.1.0`, source: 'fnm' }));
function fail(code: NodeProjectsErrorCode, message: string): never { throw { code, message }; }
export function resetNodeProjects() { generation++; projects = []; runs = {}; history = new Map(); revision = 1; serial = 0; options = {}; }
export function configure(value: typeof options) { options = { ...options, ...value }; event('node-projects-changed', { revision: `r${revision}` }); }
export function conflict() { revision++; event('node-projects-changed', { revision: `r${revision}` }); }
export function publish(run: NodeRunStatus) { event('node-project-run-status', run); }
function inspection(root: string): NodeProjectInspection {
  if (options.inspectionError || root.includes('missing')) fail('invalidManifest', 'package.json could not be read. Choose a folder with a valid manifest.');
  return { root, name: root.split('/').pop() || 'Node app', scripts: { dev: 'node server.cjs', build: 'node build.cjs', fail: 'node failure.cjs', exit: 'node exit.cjs' }, packageManager: { name: options.manager ?? 'npm', declaration: options.manager ? `${options.manager}@1.0.0` : null, lockfiles: ['package-lock.json'], npmSupported: !options.manager || options.manager === 'npm' }, versionHints: { nodeVersion: '22', nvmrc: null, enginesNode: '>=22' }, manifestRevision: 'manifest-1', warnings: [], runtimes: options.noRuntimes ? [] : runtimes, suggestedRuntimeId: options.noRuntimes ? null : 'node-22', runtimeError: null };
}
function append(run: NodeRunStatus, stream: NodeLogRecord['stream'], text: string) {
  const rows = history.get(run.runId) ?? [];
  rows.push({ runId: run.runId, cursor: rows.length + 1, time: Date.now(), stream, text }); history.set(run.runId, rows);
}
function update(run: NodeRunStatus, state: NodeRunStatus['state']) { run.state = state; run.revision++; event('node-project-run-status', structuredClone(run)); }
function checkRevision(value: unknown) { if (value !== `r${revision}`) fail('conflict', 'The registry changed.'); }
async function stop(request: Record<string, unknown>) {
  const run = runs[String(request.id)];
  if (!run) fail('runNotFound', 'No managed run exists.');
  if (options.staleStop) { options.staleStop = false; run.runId = (++serial).toString(16).padStart(32, '0'); run.createdAt++; run.revision = 1; event('node-project-run-status', structuredClone(run)); fail('staleRun', 'This stop refers to an older run.'); }
  if (run.runId !== request.runId) fail('staleRun', 'This command refers to an older run.');
  update(run, 'stopping'); await new Promise(resolve => setTimeout(resolve, 70));
  run.endedAt = Date.now(); update(run, 'stopped'); append(run, 'system', 'Managed run ended.\n'); return run;
}
export async function nodeRequest(command: string, args: Record<string, unknown>, emit: Emit, platform: string): Promise<unknown> {
  event = emit;
  const request = (args.request ?? {}) as Record<string, unknown>;
  if (command === 'node_projects_snapshot') return { supported: platform === 'macOS', revision: `r${revision}`, registryError: options.registryError ? { code: 'registryUnavailable', message: 'Saved Node registrations could not be read. Your files were kept.' } : null, projects: options.registryError ? [] : projects.map(config => ({ config, rootAvailable: !options.missingRoot, manifestAvailable: true, runtimeAvailable: !options.missingRuntime })) };
  if (command === 'node_project_runs') return { supported: platform === 'macOS', runs: Object.values(runs) };
  if (command === 'node_project_inspect') return inspection(String(request.root));
  if (command === 'node_project_save') {
    checkRevision(request.expectedRevision);
    const config = request.config as Record<string, string | null>;
    if (projects.some(project => project.id !== config.id && project.root === config.root)) fail('invalidProject', 'This project folder is already registered.');
    if (activeRun(runs[String(config.id)])) fail('projectBusy', 'Stop this project before editing.');
    const runtime = runtimes.find(runtime => runtime.id === config.runtimeId);
    if (!runtime) fail('runtimeUnavailable', 'Choose an installed runtime.');
    const project: NodeProjectConfig = { id: config.id ?? (++serial).toString(16).padStart(32, '0'), name: config.name!, root: config.root!, script: config.script!, runtime: { nodePath: runtime.nodePath, version: runtime.version }, openUrl: config.openUrl };
    projects = [...projects.filter(item => item.id !== project.id), project]; revision++;
    event('node-projects-changed', { revision: `r${revision}` }); return { revision: `r${revision}`, project };
  }
  if (command === 'node_project_prepare_runtime') {
    if (options.requirementConflict) fail('requirementConflict', 'Selected Node version does not satisfy engines.node.');
    return {};
  }
  if (command === 'node_project_remove') {
    checkRevision(request.expectedRevision);
    if (activeRun(runs[String(request.id)])) fail('projectBusy', 'Stop the project first.');
    projects = projects.filter(item => item.id !== request.id); revision++;
    event('node-projects-changed', { revision: `r${revision}` }); return { revision: `r${revision}` };
  }
  if (command === 'node_project_stop') return stop(request);
  if (command === 'node_project_restart') { await stop(request); command = 'node_project_start'; }
  if (command === 'node_project_start') {
    checkRevision(request.expectedRevision);
    if (!request.confirmed) fail('confirmationRequired', 'Confirm script execution.');
    if (options.missingRuntime) fail('runtimeUnavailable', 'The selected runtime is missing.');
    if (options.requirementConflict) fail('requirementConflict', 'Selected Node version does not satisfy engines.node.');
    const project = projects.find(project => project.id === request.id);
    if (!project) fail('notFound', 'This project is no longer registered.');
    if (activeRun(runs[project.id])) fail('projectBusy', 'This project already has a managed run.');
    const run: NodeRunStatus = { projectId: project.id, runId: (++serial).toString(16).padStart(32, '0'), revision: 1, state: 'starting', createdAt: Date.now(), startedAt: null, endedAt: null, exitCode: null, signal: null, error: null, forcedCleanup: false, runtime: project.runtime };
    runs[project.id] = run; history.set(run.runId, []); event('node-project-run-status', structuredClone(run));
    const initial = structuredClone(run), currentGeneration = generation;
    setTimeout(() => {
      if (generation !== currentGeneration || run.state !== 'starting') return;
      run.startedAt = Date.now(); update(run, 'running');
      append(run, 'stdout', `Node ${run.runtime.version} café 日本\n<script>plain text</script>\nline two\n`); append(run, 'stderr', 'controlled stderr\n');
      if (['fail', 'exit'].includes(project.script)) { run.exitCode = project.script === 'fail' ? 7 : 0; run.endedAt = Date.now(); update(run, project.script === 'fail' ? 'failed' : 'exited'); }
    }, 140);
    return initial;
  }
  if (command === 'node_project_logs') {
    if (options.logsError) fail('logUnavailable', 'Retained log history is unavailable.');
    const rows = history.get(String(request.runId)) ?? [];
    const records = rows.filter(row => row.cursor > Number(request.afterCursor)).slice(0, 256);
    return { records, nextCursor: records.at(-1)?.cursor ?? request.afterCursor, truncated: !!options.truncated, eof: !activeRun(runs[String(request.id)]), error: null };
  }
  throw new Error('Unsupported Node Projects fixture command.');
}

export function flood(start: number) {
  const run = Object.values(runs)[0];
  if (run) for (let index = start; index < start + 500; index++) append(run, 'stdout', `burst-${index} 日本\n`);
}

// Native creation hands registration to the existing workspace; never Start.
export function addCreatedProject(project: NodeProjectConfig, emit: Emit) {
  if (projects.some(existing => existing.id === project.id)) return;
  projects.push(project); revision++; emit('node-projects-changed', { revision: `r${revision}` });
}
