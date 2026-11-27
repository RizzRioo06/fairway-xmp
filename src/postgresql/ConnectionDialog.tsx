import { useRef, useState } from 'react';
import Dialog from '../data/Dialog';
import { needsClientRuntime, clientRuntimeExplanation } from './runtime-guidance';
import { credentialStoreName } from './credential-store';
import type { ConnectRequest, Connection, Problem } from './workspace-model';
import type { Profile, SaveProfile } from './profiles';
import type { ManagedPostgresSnapshot } from './types';
function lastSource(): 'managed' | 'external' {
  try { return localStorage.getItem('fairway.pg.lastSource') === 'managed' ? 'managed' : 'external'; } catch { return 'external'; }
}
export default function ConnectionDialog({ managed, platform, initial, profile, editing = false, database, busy, error, onClose, onConnect, onPrepareRuntime }: {
  managed: ManagedPostgresSnapshot | null; platform: string; initial?: Connection; profile?: Profile; editing?: boolean; database?: string;
  onPrepareRuntime: () => Promise<boolean>;
  busy: boolean; error: Problem | null; onClose: () => void;
  onConnect: (request: ConnectRequest, save?: SaveProfile, saveOnly?: boolean) => Promise<boolean>;
}) {
  const [runtimeRecovery, setRuntimeRecovery] = useState(false);
  const runtimeMissing = needsClientRuntime(error);
  const storeName = credentialStoreName(platform);
  const [kind, setKind] = useState(platform === 'windows' || profile ? 'external' : initial?.source.kind ?? lastSource());
  const [db, setDb] = useState(database ?? profile?.database ?? initial?.session.database ?? 'postgres');
  const [host, setHost] = useState(profile?.host ?? (initial?.source.kind === 'external' ? initial.source.host : '127.0.0.1'));
  const [port, setPort] = useState(profile ? String(profile.port) : initial?.source.kind === 'external' ? String(initial.source.port) : '5432');
  const [user, setUser] = useState(profile?.user ?? (initial?.source.kind === 'external' ? initial.source.user : ''));
  const [save, setSave] = useState(!!profile || !initial);
  const [name, setName] = useState(profile?.name ?? '');
  const [remember, setRemember] = useState(profile?.rememberPassword ?? false);
  const [replace, setReplace] = useState(false);
  const [consent, setConsent] = useState(false);
  const password = useRef<HTMLInputElement>(null);
  const id = useRef(profile?.id ?? crypto.randomUUID().replaceAll('-', ''));
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  const saving = kind === 'external' && save;
  const keepingPassword = !!profile?.rememberPassword && remember && !replace;
  const valid = !!db.trim() && bytes(db) <= 63 && consent && (kind === 'managed' ? !!managed?.clusterId && !!managed.running
    : !!user.trim() && bytes(user) <= 63 && Number.isInteger(Number(port)) && Number(port) > 0 && Number(port) <= 65535 && (!saving || !!name.trim() && bytes(name) <= 160));
  return <Dialog title={editing ? 'Edit saved connection' : profile ? `Connect to ${profile.name}` : 'Connect to PostgreSQL'} submit={editing ? 'Save changes' : saving ? 'Save & connect' : 'Connect'} busy={busy} valid={valid} onClose={onClose} onSubmit={() => {
    if (!valid) return;
    const secret = password.current?.value ?? ''; if (password.current) password.current.value = '';
    const target = kind === 'managed' ? { kind: 'managed' as const, clusterId: managed!.clusterId! }
      : { kind: 'external' as const, host, port: Number(port), user, password: secret, transport: 'loopback' as const };
    const metadata: SaveProfile | undefined = saving ? { confirmed: true, profile: { id: id.current, revision: profile?.revision ?? '', name, host, port: Number(port), user, database: db, rememberPassword: remember }, ...(remember && !keepingPassword ? { password: secret } : {}) } : undefined;
    const request: ConnectRequest = keepingPassword && profile && !saving ? { confirmed: true, database: db, target: { kind: 'saved', profile } } : { confirmed: true, database: db, target };
    void onConnect(request, metadata, editing);
  }}>
    {error && <div className="pg-error" role="alert"><strong>{runtimeMissing ? 'Prepare PostgreSQL connection tools' : editing ? 'Connection could not be saved' : 'Could not connect'}</strong><p>{runtimeMissing ? clientRuntimeExplanation : error.text}</p><p>{runtimeMissing ? 'Keep these connection details. Prepare the tools below, then connect again.' : 'Your connection details are kept here. Update them and retry.'}</p>{(runtimeMissing || runtimeRecovery) && <button type="button" className="primary" disabled={busy} onClick={() => { setRuntimeRecovery(true); void onPrepareRuntime().then(ok => { if (ok) setRuntimeRecovery(false); }); }}>Prepare connection tools</button>}</div>}
    {!profile && <label>Connection source<select aria-label="Connection source" value={kind} onChange={e => {
      const next = e.target.value as 'managed' | 'external'; setKind(next); setConsent(false);
      try { localStorage.setItem('fairway.pg.lastSource', next); } catch { /* Preference storage is optional. */ }
      if (password.current) password.current.value = '';
    }}><option value="external">Existing PostgreSQL server</option>{platform === 'macos' && <option value="managed">Fairway managed PostgreSQL</option>}</select></label>}
    {kind === 'external' && <p className="db-hint">Connect to your running local server. Fairway will not start, stop or reconfigure it.</p>}
    <label>Database<input aria-label="Connection database" value={db} onChange={e => setDb(e.target.value)} required maxLength={63} /></label>
    {kind === 'managed' ? <p className="db-hint">{managed?.running ? `Uses your managed cluster on port ${managed.port} and its Keychain credential.` : 'Initialize and start a managed cluster first.'}</p> : <>
      <div className="pg-form-pair"><label>Host<select aria-label="PostgreSQL host" value={host} onChange={e => { setHost(e.target.value); if (keepingPassword) { setRemember(false); setReplace(true); } }}><option>127.0.0.1</option><option>::1</option></select></label><label>Port<input aria-label="PostgreSQL port" type="number" min={1} max={65535} value={port} onChange={e => { setPort(e.target.value); if (keepingPassword) { setRemember(false); setReplace(true); } }} /></label></div>
      <label>User<input aria-label="PostgreSQL user" value={user} onChange={e => { setUser(e.target.value); if (keepingPassword) { setRemember(false); setReplace(true); } }} autoComplete="off" maxLength={63} required /></label>
      {keepingPassword ? <div className="pg-secret-status"><span>Password saved in {storeName}</span><button type="button" className="secondary" onClick={() => setReplace(true)}>Change password</button></div>
        : <label>Password<input ref={password} aria-label="PostgreSQL password" type="password" autoComplete="off" maxLength={4096} /><span className="db-hint">{remember ? `Stored securely in ${storeName}.` : 'Used for this session only.'} Leave blank for an empty password.</span></label>}
      <div className="pg-save-options">
        <label className="pg-check"><input type="checkbox" checked={save} disabled={!!profile} onChange={e => setSave(e.target.checked)} />Save this connection for next time</label>
        {save && <><label>Connection name<input aria-label="Connection name" value={name} onChange={e => setName(e.target.value)} placeholder="e.g. Local development" maxLength={160} required /></label><label className="pg-check"><input type="checkbox" checked={remember} onChange={e => setRemember(e.target.checked)} />Remember password in {storeName}</label></>}
      </div>
      <p className="db-hint">Local connections only (127.0.0.1 or ::1). Connection details are saved on this device; passwords are never written to settings.</p>
    </>}
    <label className="pg-check"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)} />{editing ? 'Save these connection settings' : 'Connect to this database'}</label>
  </Dialog>;
}
