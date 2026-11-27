import { Bookmark, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react';
import type { Profile } from './profiles';
import { credentialStoreName } from './credential-store';
import type { Connection } from './workspace-model';
export default function SavedConnections({ profiles, platform, connections, connectingId, atCapacity, busy, loading, onConnect, onEdit, onRemove, onNew, onRefresh }: {
  profiles: Profile[]; platform: string; connections: Connection[]; connectingId: string; atCapacity: boolean; busy: boolean; loading: boolean; onConnect: (p: Profile) => void; onEdit: (p: Profile) => void;
  onRemove: (p: Profile) => void; onNew: () => void; onRefresh: () => void;
}) {
  return <section className="pg-saved settings-card" aria-label="Saved PostgreSQL connections">
    <div className="card-title"><div><h2><Bookmark size={16} />Saved connections</h2><p className="db-hint">Saved on this device. Connect when you need them.</p></div><div className="pg-actions"><button className="secondary" disabled={busy || loading} aria-label="Refresh saved connections" onClick={onRefresh}><RefreshCw size={14} /></button><button className="primary" disabled={busy || loading || atCapacity} onClick={onNew}><Plus size={14} />New connection</button></div></div>
    {loading ? <p role="status" className="db-hint">Loading saved connections…</p> : profiles.length ? <div className="pg-saved-list">{profiles.map(p => {
      const session = connections.find(c => c.source.kind === 'external' && c.source.host === p.host && c.source.port === p.port && c.source.user === p.user && c.session.database === p.database && c.access === 'authenticated');
      return <div className="pg-saved-row" key={p.id}>
      <div className="pg-saved-description"><strong>{p.name}{(connectingId === p.id || session) && <span className="pg-connection-status" role="status">{connectingId === p.id ? 'Connecting…' : 'Connected'}</span>}</strong><span>{p.user} · {p.host}:{p.port} / {p.database}</span><small>{p.rememberPassword ? `Password in ${credentialStoreName(platform)}` : 'Password requested on connect'}</small></div>
      <div className="pg-actions"><button className="primary" disabled={busy || atCapacity} onClick={() => onConnect(p)}>Connect</button><button className="secondary" disabled={busy} aria-label={`Edit ${p.name}`} onClick={() => onEdit(p)}><Pencil size={14} /></button><button className="secondary" disabled={busy} aria-label={`Remove ${p.name}`} onClick={() => onRemove(p)}><Trash2 size={14} /></button></div>
    </div>; })}</div> : <div className="pg-saved-empty"><strong>Connect once. Keep the details.</strong><p>Add your running PostgreSQL server and save it here. Choose whether to remember its password.</p></div>}
  </section>;
}
