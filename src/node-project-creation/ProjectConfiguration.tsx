import { FolderOpen, RefreshCw } from 'lucide-react';
import { useId } from 'react';
import { nameError } from './model';
import type { NodeCreationPlan, NodeTemplate, TemplateCatalogSnapshot } from './types';
export interface CreationDraft { name: string; parent: string; runtime: string; template: string }
export default function ProjectConfiguration({ draft, catalog, template, plan, pending, onChange, onFolder, onRefresh }: { draft: CreationDraft; catalog: TemplateCatalogSnapshot; template: NodeTemplate; plan: NodeCreationPlan | null; pending: boolean; onChange: (value: CreationDraft) => void; onFolder: () => void; onRefresh: () => void }) {
  const id = useId(), invalid = draft.name ? nameError(draft.name) : null;
  return <div className="node-project-form">
    <p>{template.title} · Required Node.js: <code>{template.nodeRange}</code>. Compatibility and paired npm are checked by Fairway before review.</p>
    <label htmlFor={`${id}-name`}>Project name<input id={`${id}-name`} value={draft.name} maxLength={64} autoComplete="off" spellCheck={false} disabled={pending} aria-invalid={!!invalid} aria-describedby={`${id}-name-help`} placeholder="my-node-app" onChange={event => onChange({ ...draft, name: event.target.value })} /></label>
    <small id={`${id}-name-help`}>{invalid ?? 'Lowercase letters, digits and hyphens. Fairway creates a new folder with this name.'}</small>
    <div className="site-field"><label htmlFor={`${id}-parent`}>Parent folder</label><span className="root-input"><input id={`${id}-parent`} value={draft.parent} readOnly placeholder="Choose where to create the project" /><button type="button" className="secondary" disabled={pending} onClick={onFolder}><FolderOpen size={14} />Choose parent folder…</button></span></div>
    <label htmlFor={`${id}-runtime`}>Node.js runtime<select id={`${id}-runtime`} aria-label="Node.js runtime" value={draft.runtime} disabled={pending} onChange={event => onChange({ ...draft, runtime: event.target.value })}><option value="">Choose an installed runtime</option>{catalog.runtimes.map(runtime => <option key={runtime.id} value={runtime.id}>{runtime.version} · {runtime.source}</option>)}</select></label>
    {!catalog.runtimes.length && <p role="status">No installed runtimes found. Install a Node.js version matching the template requirements in Node tools, then refresh.</p>}
    {catalog.discoveryNote && <p>{catalog.discoveryNote}</p>}
    {catalog.runtimeError && <p role="alert">{catalog.runtimeError}</p>}
    <button type="button" className="text-link" disabled={pending} onClick={onRefresh}><RefreshCw size={13} />Refresh runtimes and catalog</button>
    {plan ? <p className="creation-verified" role="status">Verified Node {plan.tools.nodeVersion} with paired npm {plan.tools.npmVersion}. Review the setup to continue.</p> : <p>Check setup to validate the destination, runtime and paired npm. Checking does not create files or download packages.</p>}
  </div>;
}
