import type { NodeCreationPlan } from './types';
export default function CreationReview({ plan, expired, confirmed, onConfirm }: { plan: NodeCreationPlan; expired: boolean; confirmed: boolean; onConfirm: (value: boolean) => void }) {
  return <div className="creation-review">
    <dl><div><dt>Destination</dt><dd>{plan.destination}</dd></div><div><dt>Template</dt><dd>{plan.template.title} · revision {plan.template.revision}</dd></div><div><dt>Runtime</dt><dd>Node {plan.tools.nodeVersion} · npm {plan.tools.npmVersion}</dd></div></dl>
    <details><summary>{Object.keys(plan.files).length} generated files · {Math.ceil(Object.values(plan.files).reduce((sum, file) => sum + file.bytes, 0) / 1024)} KiB before dependencies</summary><ul>{Object.keys(plan.files).map(path => <li key={path}><code>{path}</code></li>)}</ul></details>
    {plan.warnings.length > 0 && <div className="notice" role="note"><strong>Review notes</strong><ul>{plan.warnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul></div>}
    <h4>Dependencies</h4>{plan.template.dependencies.length ? <table><caption>Direct packages pinned by this template</caption><thead><tr><th>Package</th><th>Version</th><th>Use</th></tr></thead><tbody>{plan.template.dependencies.map(dependency => <tr key={dependency.name}><td><code>{dependency.name}</code></td><td>{dependency.version}</td><td>{dependency.development ? 'Development' : 'Runtime'}</td></tr>)}</tbody></table> : <p>No third-party packages are required by this template. npm still verifies its lockfile.</p>}
    <p>npm will download packages from <code>{plan.installation.registry}</code> into a private staging folder. The reviewed lockfile records exact versions and integrity checks. Installation lifecycle scripts are disabled.</p>
    <p>Fairway verifies the files, publishes the new folder without replacing an existing one, then registers the project. Your Node.js default stays unchanged. The development server will not start automatically.</p>
    <label className="creation-consent"><input type="checkbox" checked={confirmed} disabled={expired} onChange={event => onConfirm(event.target.checked)} /><span>I approve creating these files and installing the listed dependencies with npm.</span></label>
  </div>;
}
