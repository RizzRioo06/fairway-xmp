// UI-test only. Durable native jobs are simulated across WebView reload with
// session storage and elapsed fixture time. No files, npm or processes run.
import catalogData from '../../src-tauri/src/node_project_creation/catalog.json';
import type { CreationErrorCode, CreationJobStatus, NodeCreationPlan, TemplateCatalogSnapshot } from '../node-project-creation/types';
import type { NodeLogRecord, NodeProjectConfig } from '../node-projects/types';
import { addCreatedProject } from './node-projects';
const nativeCatalog: TemplateCatalogSnapshot['catalog'] = JSON.parse(JSON.stringify(catalogData));
type Emit = (event: string, payload: unknown) => void;
type Options = { parent?: string | null; noRuntimes?: boolean; conflict?: boolean; planError?: CreationErrorCode | null; expireValidate?: boolean; expiredPlan?: boolean; duration?: number; failAt?: 'network' | 'integrity' | 'registration' | 'lock'; emptyCatalog?: boolean; logsDelay?: number; flood?: boolean };
type Entry = { job: CreationJobStatus; plan: NodeCreationPlan; started: number; duration: number; failAt?: Options['failAt'] };
type State = { options: Options; plans: Record<string, NodeCreationPlan>; jobs: Record<string, Entry>; stats: { starts: number; plans: number; validations: number; cancels: number; recoveries: number } };
const key = 'fairway.uiCreationFixture';
const empty = (): State => ({ options: {}, plans: {}, jobs: {}, stats: { starts: 0, plans: 0, validations: 0, cancels: 0, recoveries: 0 } });
function load(): State { try { return JSON.parse(sessionStorage.getItem(key) ?? 'null') ?? empty(); } catch { return empty(); } }
let state = load();
function save() { sessionStorage.setItem(key, JSON.stringify(state)); }
const id = () => crypto.randomUUID().replaceAll('-', '');
const runtimes = [22, 24, 20].map(major => ({ id: `creation-node-${major}`, nodePath: `/controlled/template-node${major}/bin/node`, version: major === 22 ? 'v22.23.4' : major === 24 ? 'v24.20.0' : 'v20.19.0', source: 'controlled fixture' }));
function fail(code: CreationErrorCode, message: string): never { throw { code, message }; }
export function configure(options: Options) { state.options = { ...state.options, ...options }; save(); }
export function resetCreation() { state = empty(); sessionStorage.removeItem(key); sessionStorage.removeItem('fairway.nodeCreation.selection'); }
export function stats() { return structuredClone(state.stats); }
export function folder() { return state.options.parent === undefined ? '/controlled/Node café 日本 projects' : state.options.parent; }
function registration(entry: Entry): NodeProjectConfig { const { job, plan } = entry; return { id: job.jobId, name: plan.projectName, root: job.destination, script: 'dev', runtime: job.installation!.runtime, openUrl: null }; }
function bump(entry: Entry) { entry.job.revision++; entry.job.installation!.revision++; }
function advance(emit: Emit) {
  for (const entry of Object.values(state.jobs)) {
    const { job } = entry, install = job.installation!;
    if (job.active) {
      const fraction = (Date.now() - entry.started) / entry.duration;
      const next = fraction < .12 ? 'preparing' : fraction < .52 ? 'installing' : fraction < .68 ? 'verifying' : fraction < .84 ? 'publishing' : fraction < 1 ? 'registering' : 'complete';
      const failNow = entry.failAt === 'registration' ? fraction >= 1 : !!entry.failAt && fraction >= .52;
      if (failNow) {
        install.state = 'failed'; install.exitCode = entry.failAt === 'registration' ? 0 : 1; job.active = false; job.recoveryRequired = true;
        job.phase = entry.failAt === 'registration' ? 'needsRegistration' : 'ready'; install.ciCompleted = entry.failAt === 'registration';
        install.error = { code: entry.failAt === 'registration' ? 'registrationFailed' : entry.failAt === 'lock' ? 'invalidLockfile' : 'installationFailed', message: entry.failAt === 'registration' ? 'The generated folder was published, but the registry could not be saved.' : entry.failAt === 'integrity' ? 'Dependency integrity verification failed (EINTEGRITY).' : entry.failAt === 'lock' ? 'The reviewed lockfile is missing or invalid.' : 'The npm registry could not be reached. Your staging files were kept.' }; bump(entry);
      } else if (install.progress !== next) {
        install.progress = next; install.state = next === 'preparing' ? 'starting' : next === 'complete' ? 'completed' : 'installing';
        install.ciCompleted = ['publishing', 'registering', 'complete'].includes(next);
        job.phase = next === 'preparing' ? 'staging' : next === 'publishing' ? 'publishing' : next === 'registering' ? 'needsRegistration' : next === 'complete' ? 'completed' : 'ready';
        if (next === 'complete') { job.active = false; job.recoveryRequired = false; job.projectId = job.jobId; install.exitCode = 0; }
        bump(entry);
      }
    }
    if (job.phase === 'completed' && install.state === 'completed') addCreatedProject(registration(entry), emit);
  }
  save();
}
export function interrupt() { for (const entry of Object.values(state.jobs)) if (entry.job.active) { entry.job.active = false; entry.job.recoveryRequired = true; entry.job.installation!.state = 'interrupted'; bump(entry); } save(); }
export async function creationRequest(command: string, args: Record<string, unknown>, emit: Emit, platform: string): Promise<unknown> {
  const request = (args.request ?? {}) as Record<string, unknown>;
  advance(emit);
  if (command === 'node_project_templates') return { supported: platform === 'macOS', catalogRevision: 'catalog-4', catalog: { ...nativeCatalog, templates: state.options.emptyCatalog ? [] : nativeCatalog.templates }, runtimes: state.options.noRuntimes ? [] : runtimes, runtimeError: null, discoveryNote: 'Controlled fixture inventory.' } satisfies TemplateCatalogSnapshot as TemplateCatalogSnapshot;
  if (command === 'node_project_creation_jobs') return { supported: platform === 'macOS', jobs: Object.values(state.jobs).map(entry => entry.job), errors: [] };
  if (command === 'node_project_creation_plan') {
    state.stats.plans++; save();
    if (state.options.planError) fail(state.options.planError, `Controlled ${state.options.planError}: setup could not be validated.`);
    if (request.catalogRevision !== 'catalog-4') fail('catalogChanged', 'Refresh the catalog.');
    const template = nativeCatalog.templates.find(template => template.id === request.templateId);
    if (!template) fail('invalidTemplate', 'Choose a template.');
    const name = String(request.projectName), parent = String(request.parent);
    if (!/^[a-z][a-z0-9-]{0,63}$/.test(name) || name.endsWith('-') || name === 'con') fail('invalidName', 'This project name is invalid or reserved.');
    if (!parent.startsWith('/') || parent.includes('..')) fail('invalidDestination', 'Choose an existing absolute parent folder.');
    if (name === 'already-exists') fail('destinationExists', 'The destination already exists.');
    const runtime = state.options.noRuntimes ? undefined : runtimes.find(runtime => runtime.id === request.runtimeId);
    if (!runtime) fail('runtimeUnavailable', 'The selected runtime is missing.');
    if (runtime.id.endsWith('-20') || state.options.conflict) fail('requirementConflict', 'Selected Node version does not satisfy the template engine range.');
    const plan = { schemaVersion: 1, id: id(), digest: id() + id(), catalogRevision: 'catalog-4', createdAt: Date.now(), expiresAt: Date.now() + (state.options.expiredPlan ? -1 : 600000), projectName: name, parent, destination: `${parent}/${name}`, template, tools: { runtimeId: runtime.id, nodePath: runtime.nodePath, nodeVersion: runtime.version, npmCliPath: '/controlled/npm/bin/npm-cli.js', npmVersion: runtime.id.endsWith('-24') ? '11.19.0' : '10.9.9', revision: 'tools-1' }, installation: { packageManager: 'npm', intendedCommand: 'ci', ignoreScripts: true, registry: 'https://registry.npmjs.org/', cacheScope: 'appOwned', dependencyLock: 'npmGenerated', installationAvailable: true }, files: Object.fromEntries(template.intendedFiles.map(file => [file, { bytes: 200, sha256: 'a'.repeat(64) }])), warnings: ['Dry run only.'] } as NodeCreationPlan;
    state.plans[plan.id] = plan; save(); return plan;
  }
  if (command === 'node_project_creation_validate_plan') {
    state.stats.validations++; save();
    if (state.options.expireValidate) { state.options.expireValidate = false; save(); fail('planExpired', 'The reviewed creation plan expired.'); }
    const plan = state.plans[String(request.planId)]; if (!plan || plan.digest !== request.digest) fail('planTampered', 'Review is unavailable.'); if (plan.expiresAt <= Date.now()) fail('planExpired', 'Review expired.'); return plan;
  }
  if (command === 'node_project_install_start') {
    if (!request.confirmed || !request.installConfirmed) fail('confirmationRequired', 'Confirm both file creation and installation.');
    const plan = state.plans[String(request.planId)]; if (!plan || plan.digest !== request.digest) fail('planTampered', 'Review is unavailable.');
    if (state.jobs[plan.id]) fail('planConsumed', 'This plan was already submitted.');
    if (Object.values(state.jobs).some(entry => entry.job.active)) fail('installBusy', 'An installation is already running.');
    state.stats.starts++;
    state.jobs[plan.id] = { plan, started: Date.now(), duration: state.options.duration ?? 2100, failAt: state.options.failAt, job: { jobId: plan.id, revision: 1, phase: 'staging', destination: plan.destination, stagingPath: `${plan.parent}/.fairway-node-${plan.id}`, projectId: null, error: null, active: true, recoveryRequired: false, installation: { jobId: id(), creationId: plan.id, runId: id(), revision: 1, state: 'starting', progress: 'preparing', runtime: { nodePath: plan.tools.nodePath, version: plan.tools.nodeVersion }, lockfileSha256: plan.files['package-lock.json'].sha256, registry: plan.installation.registry, ciCompleted: false, cancelRequested: false, exitCode: null, error: null } } }; save(); return state.jobs[plan.id].job.installation;
  }
  const entry = Object.values(state.jobs).find(entry => entry.job.installation?.jobId === request.jobId);
  if (!entry) fail('jobUnavailable', 'No native fixture job matches this ID.');
  const install = entry.job.installation!;
  if (command === 'node_project_install_status') return install;
  if (command === 'node_project_install_cancel') {
    if (request.expectedRevision !== install.revision) fail('staleInstallation', 'Setup status changed. Refresh before cancelling.');
    if (!entry.job.active) fail('cancellationTooLate', 'Setup is no longer active.');
    state.stats.cancels++; install.cancelRequested = true; install.state = 'cancelled'; entry.job.active = false; entry.job.recoveryRequired = true; bump(entry); save(); return install;
  }
  if (command === 'node_project_install_recover') {
    if (!request.confirmed || request.expectedRevision !== install.revision) fail('staleInstallation', 'Refresh before recovery.');
    if (!install.ciCompleted || request.action !== 'finalize' || install.state === 'completed') fail('recoveryUnsafe', 'No verified completed installation can be finalized.');
    state.stats.recoveries++; install.state = 'completed'; install.progress = 'complete'; entry.job.phase = 'completed'; entry.job.projectId = entry.job.jobId; entry.job.active = false; entry.job.recoveryRequired = false; install.error = null; bump(entry); addCreatedProject(registration(entry), emit); save(); return install;
  }
  if (command === 'node_project_install_logs') {
    if (state.options.logsDelay) await new Promise(resolve => setTimeout(resolve, state.options.logsDelay));
    const rows: NodeLogRecord[] = state.options.flood ? Array.from({ length: 1300 }, (_, index) => ({ runId: install.runId, cursor: index + 1, stream: index % 2 ? 'stdout' : 'stderr', time: entry.started, text: `${entry.plan.projectName} burst-${index} 日本 ${'x'.repeat(300)}\n` })) : [{ runId: install.runId, cursor: 1, time: entry.started, stream: 'stdout', text: `${entry.plan.projectName}: npm install café 日本\n<script>plain text</script>\n` }, { runId: install.runId, cursor: 2, time: entry.started, stream: 'stderr', text: install.error?.message ?? 'controlled npm output\n' }];
    const records = rows.filter(row => row.cursor > Number(request.afterCursor)).slice(0, 100);
    return { records, nextCursor: records.at(-1)?.cursor ?? request.afterCursor, eof: !entry.job.active && (!records.length || records.at(-1)?.cursor === rows.length), truncated: !!state.options.flood, error: null };
  }
  throw new Error(`Unsupported creation fixture command: ${command}`);
}
