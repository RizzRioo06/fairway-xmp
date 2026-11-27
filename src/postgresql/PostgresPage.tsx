import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { Database, LoaderCircle, Play, RefreshCw, Square, Unplug } from 'lucide-react';
import { canControl } from '../bridge';
import Dialog from '../data/Dialog';
import TableWorkspace from '../data/TableWorkspace';
import Workspace, { MaximizeWorkspace } from '../Workspace';
import { PostgresWorkspace, type Connection, type Problem, type ServiceAction, type Pane } from './workspace-model';
import { workspacePort } from './workspace-port';
import type { PgPage } from './explorer-types';
import PgResult from './PgResult';
import ConnectionDialog from './ConnectionDialog';
import { needsClientRuntime, clientRuntimeExplanation } from './runtime-guidance';
import SavedConnections from './SavedConnections';
import { listProfiles, saveProfile, removeProfile, type Profile, type SaveProfile } from './profiles';
import { problem, type ConnectRequest } from './workspace-model';
import './postgresql.css';
const paneNames: Record<Pane, string> = { rows: 'Rows', columns: 'Columns', indexes: 'Indexes', view: 'View definition', query: 'SQL editor' };
const filterNames = { contains: 'Contains text', eq: 'Equals', ne: 'Does not equal', lt: 'Less than', lte: 'Less than or equal', gt: 'Greater than', gte: 'Greater than or equal', isNull: 'Is NULL (no value)', notNull: 'Is not NULL' } as const;
const serviceNames: Record<ServiceAction, string> = { provision: 'Prepare runtime', initialize: 'Initialize new cluster', start: 'Start managed PostgreSQL', stop: 'Stop managed PostgreSQL', restart: 'Restart managed PostgreSQL' };
function ErrorMessage({ error, onPrepareRuntime }: { error: Problem; onPrepareRuntime?: () => Promise<boolean> }) { return <div className="pg-error" role="alert"><strong>{error.code === 'timedOut' ? 'Operation timed out' : error.code === 'cancelled' ? 'Operation cancelled' : error.code === 'busy' ? 'PostgreSQL is busy — retry when the current operation finishes' : 'PostgreSQL needs attention'}</strong><p>{needsClientRuntime(error) ? clientRuntimeExplanation : error.text}</p>{needsClientRuntime(error) && onPrepareRuntime && <button className="primary" onClick={() => void onPrepareRuntime()}>Prepare connection tools</button>}{error.sqlState && <p>SQLSTATE: <code>{error.sqlState}</code>{error.position !== null && error.position !== undefined ? ` · Position ${error.position}` : ''}</p>}{error.outcome === 'unknown' ? <p><strong>Outcome unknown. Check the database before retrying. Cancellation does not prove rollback.</strong></p> : error.outcome && <p>Outcome: {error.outcome === 'notStarted' ? 'Not started' : error.outcome === 'committed' ? 'Committed' : 'Not committed; nontransactional effects may remain'}.</p>}</div>; }
export default function PostgresPage({ active, busy, platform, embedded = false }: { active: boolean; busy: boolean; platform: string; embedded?: boolean }) {
  const [controller] = useState(() => new PostgresWorkspace(workspacePort));
  const state = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const [connect, setConnect] = useState<{ initial?: Connection; database?: string; profile?: Profile; editing?: boolean } | null>(null);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profilesLoading, setProfilesLoading] = useState(true);
  const [profileBusy, setProfileBusy] = useState(false);
  const [connectingProfile, setConnectingProfile] = useState('');
  const [profileError, setProfileError] = useState<Problem | null>(null);
  const [dialogError, setDialogError] = useState<Problem | null>(null);
  const [remove, setRemove] = useState<Profile | null>(null);
  const [search, setSearch] = useState('');
  const [service, setService] = useState<ServiceAction | null>(null);
  const [port, setPort] = useState('15432');
  const [serviceConsent, setServiceConsent] = useState(false);
  const [sql, setSql] = useState('SELECT current_database(), current_user;');
  const [mode, setMode] = useState<'readOnly' | 'readWrite'>('readOnly');
  const [write, setWrite] = useState<{ sql: string; session: string } | null>(null);
  const [writeConsent, setWriteConsent] = useState(false);
  const connected = state.connections.find(c => c.session.id === state.activeId);
  const lifetime = useRef(0);
  const visible = useRef(active); visible.current = active;
  const profileRefresh = useRef(0);
  const blocked = !canControl || !!state.pending || busy || profileBusy;
  useEffect(() => { const token = ++lifetime.current; return () => { queueMicrotask(() => { if (lifetime.current === token) controller.dispose(); }); }; }, [controller]);
  useEffect(() => { if (active && platform === 'macos') void controller.status(); else { controller.cancel(); setConnect(null); setWrite(null); setService(null); } }, [active, platform, controller]);
  useEffect(() => { setMode('readOnly'); setWrite(null); setWriteConsent(false); }, [state.activeId]);
  async function refreshProfiles() {
    const token = ++profileRefresh.current;
    setProfilesLoading(true);
    try {
      const values = await listProfiles();
      if (profileRefresh.current === token) { setProfiles(values); setProfileError(null); }
      return values;
    }
    catch (error) { if (profileRefresh.current === token) setProfileError(problem(error)); }
    finally { if (profileRefresh.current === token) setProfilesLoading(false); }
  }
  useEffect(() => { if (active && platform === 'macos' && canControl) void refreshProfiles(); }, [active, platform]);
  useEffect(() => { setSearch(''); }, [state.schema, state.activeId]);
  function showConnect(value: NonNullable<typeof connect>) { setDialogError(null); setConnect(value); }
  async function submitConnection(request: ConnectRequest, save?: SaveProfile, saveOnly = false) {
    const originalTarget = request.target;
    try { localStorage.setItem('fairway.pg.lastSource', request.target.kind === 'managed' ? 'managed' : 'external'); } catch { /* Optional preference. */ }
    setProfileBusy(true); setConnectingProfile(save?.profile.id ?? (request.target.kind === 'saved' ? request.target.profile.id : '')); setDialogError(null);
    try {
      if (save) {
        let profile: Profile;
        try { profile = await saveProfile(save); } finally { save.password = undefined; }
        if (profile.rememberPassword && originalTarget.kind === 'external') originalTarget.password = '';
        setProfiles(previous => [...previous.filter(p => p.id !== profile.id), profile]);
        // Retain the committed revision if connecting fails, so Retry edits the same profile.
        setConnect(previous => previous ? { ...previous, profile } : previous);
        if (profile.rememberPassword) request = { confirmed: true, database: profile.database, target: { kind: 'saved', profile } };
      }
      if (saveOnly) { setConnect(null); return true; }
      if (!visible.current) return false;
      const ok = await controller.connect(request);
      if (ok) setConnect(null); else setDialogError(controller.snapshot().error ?? { text: 'Connection was interrupted. Retry when the current operation finishes.' });
      return ok;
    } catch (error) {
      setDialogError(problem(error));
      // Refresh a possibly committed profile after a vault cleanup/durability failure.
      const refreshed = await refreshProfiles();
      const committed = refreshed?.find(p => save && p.id === save.profile.id && p.name === save.profile.name.trim() && p.host === save.profile.host && p.port === save.profile.port && p.user === save.profile.user && p.database === save.profile.database && p.rememberPassword === save.profile.rememberPassword);
      if (committed) setConnect(previous => previous ? { ...previous, profile: committed } : previous);
      return false;
    } finally {
      if (save) save.password = undefined;
      if (originalTarget.kind === 'external') originalTarget.password = '';
      if (originalTarget.kind === 'saved') originalTarget.password = undefined;
      if (request.target.kind === 'external') request.target.password = '';
      if (request.target.kind === 'saved') request.target.password = undefined;
      setProfileBusy(false); setConnectingProfile('');
    }
  }
  async function forgetProfile(profile: Profile) {
    setProfileBusy(true);
    try { await removeProfile(profile); setRemove(null); await refreshProfiles(); }
    catch (error) { setRemove(null); await refreshProfiles(); setProfileError(problem(error)); }
    finally { setProfileBusy(false); }
  }
  async function prepareConnectionTools() {
    if (blocked) return false;
    await controller.service('provision');
    const error = controller.snapshot().serviceError;
    setDialogError(error);
    return !error;
  }
  function serviceDialog(action: ServiceAction) { setService(action); setServiceConsent(false); }
  if (platform !== 'macos') return <section className="pg-page" aria-label="PostgreSQL workspace"><div className="page-heading"><h1>PostgreSQL</h1></div><p>PostgreSQL browsing and managed services currently require an Apple Silicon Mac. They are unavailable on this platform.</p><p>Your PHP, MySQL, SQLite and Node.js tools remain available.</p></section>;
  return <section className="pg-page" aria-label="PostgreSQL workspace">
    {!embedded && <div className="page-heading"><div><h1>PostgreSQL</h1><p>Browse schemas and run explicitly authorized SQL.</p></div><MaximizeWorkspace id="postgres" title="PostgreSQL" /></div>}
    <div className="pg-intro"><div><h2>PostgreSQL</h2><p>Connect to your running local server, then choose a database, schema and table.</p></div><span className="pg-source-badge">Local server connection</span></div>
    <SavedConnections platform={platform} profiles={profiles} connections={state.connections} connectingId={connectingProfile} atCapacity={state.connections.length >= 16} busy={blocked} loading={profilesLoading} onNew={() => showConnect({})} onRefresh={() => void refreshProfiles()} onEdit={profile => showConnect({ profile, editing: true })} onRemove={setRemove} onConnect={profile => {
      if (profile.rememberPassword) void submitConnection({ confirmed: true, database: profile.database, target: { kind: 'saved', profile } }).then(ok => { if (!ok && visible.current) setConnect({ profile }); });
      else showConnect({ profile });
    }} />
    {profileError && <ErrorMessage error={profileError} />}
    <details className="pg-managed-details"><summary><span>Fairway managed PostgreSQL</span><small>{state.managed?.running ? 'Running' : state.managed?.clusterId ? 'Stopped' : 'Optional local server'}</small></summary>
    <section className="pg-service settings-card" aria-label="Managed PostgreSQL status"><div className="card-title"><h2>Fairway managed PostgreSQL</h2><button className="secondary" disabled={blocked} onClick={() => void controller.status()}><RefreshCw size={14} />Refresh status</button></div>
      <dl className="pg-health"><div><dt>Process</dt><dd>{state.managed ? state.managed.running ? 'Running' : 'Stopped' : 'Unknown'}</dd></div><div><dt>Service responsiveness</dt><dd>{({ notChecked: 'Not checked', accepting: 'Accepting connections', rejecting: 'Rejecting connections', noResponse: 'No response' })[state.managed?.responsiveness ?? 'notChecked']}</dd></div><div><dt>Authenticated access</dt><dd>{({ notChecked: 'Not checked', authenticated: 'Verified by health check', rejected: 'Rejected' })[state.managed?.authentication ?? 'notChecked']}</dd></div></dl>
      <p className="db-hint">{state.managed?.port ? `Loopback port ${state.managed.port}. ` : ''}A running process alone does not establish database access. Checks are explicit snapshots, not live monitoring.</p>
      {state.managed?.message && <p className="pg-warning">{state.managed.message}</p>}{state.serviceError && <ErrorMessage error={state.serviceError} />}
      <div className="pg-actions"><button className="secondary" disabled={blocked || !state.managed?.running} onClick={() => void controller.status(true)}>Check access</button>{state.managed?.clusterId ? <><button className="secondary" disabled={blocked || state.managed.running} onClick={() => serviceDialog('start')}><Play size={14} />Start</button><button className="secondary" disabled={blocked || !state.managed.running} onClick={() => serviceDialog('stop')}><Square size={14} />Stop</button><button className="secondary" disabled={blocked || !state.managed.running} onClick={() => serviceDialog('restart')}>Restart</button></> : <><button className="secondary" disabled={blocked} onClick={() => serviceDialog('provision')}>Prepare runtime</button><button className="secondary" disabled={blocked || !state.managed?.supported} onClick={() => serviceDialog('initialize')}>Initialize new cluster</button></>}</div>
    </section></details>
    <Workspace id="postgres" title="PostgreSQL" className="pg-workspace" busy={!!state.pending}>
      <div className="pg-connection-bar"><label>Open connection<select aria-label="PostgreSQL connection" disabled={blocked} value={state.activeId} onChange={e => void controller.select(e.target.value)}><option value="" disabled>Choose a connected database</option>{state.connections.map(c => <option key={c.session.id} value={c.session.id}>{c.source.kind === 'managed' ? 'Managed' : `External ${c.source.host}:${c.source.port}`} / {c.session.database}</option>)}</select></label><button className="primary" disabled={blocked || state.connections.length >= 16} onClick={() => showConnect({})}>Add connection</button>{connected && <button className="secondary" onClick={() => void controller.disconnect(connected.session.id)}><Unplug size={14} />Disconnect</button>}</div>
      {state.pending && <div className="pg-pending" role="status"><LoaderCircle size={16} className="spin" />{state.pending.label}{state.pending.canCancel && <button className="secondary" disabled={state.pending.cancelling} onClick={() => controller.cancel()}>{state.pending.query ? 'Cancel Query' : 'Cancel operation'}</button>}</div>}
      {state.error && <ErrorMessage error={state.error} onPrepareRuntime={prepareConnectionTools} />}{state.notice && <p className="pg-warning" role="status">{state.notice}</p>}
      {connected ? <><div className="pg-session-heading"><strong>{connected.source.kind === 'managed' ? 'Managed cluster' : 'Existing PostgreSQL server'} / {connected.session.database}</strong><span>{connected.access === 'authenticated' ? 'Connected — last request succeeded' : connected.access === 'failed' ? 'Access failed — retry or reconnect' : 'Access not checked'}</span></div>
        <div className="pg-catalog-controls"><label>Database<select aria-label="PostgreSQL database" disabled={blocked} value={connected.session.database} onChange={e => { void controller.connect({ confirmed: true, database: e.target.value, target: { kind: 'session', sessionId: connected.session.id } }); }}><option value={connected.session.database}>{connected.session.database}</option>{state.databases.filter(d => d !== connected.session.database).map(d => <option key={d}>{d}</option>)}</select></label><label>Schema<select aria-label="PostgreSQL schema" disabled={blocked} value={state.schema} onChange={e => void controller.schema(e.target.value)}><option value="" disabled>Choose a schema</option>{state.schemas.map(s => <option key={s}>{s}</option>)}</select></label><button className="secondary" disabled={blocked} onClick={() => void controller.refresh()}><RefreshCw size={14} />Refresh connection</button></div>
        <p className="pg-workspace-help">A schema groups tables within a database. Choose a schema above, then select a table or view on the left.</p>
        <TableWorkspace sidebar={<div className="db-tables"><h3>Tables &amp; views <small>({state.relations.length})</small></h3><label className="pg-table-search">Find a table<input type="search" aria-label="Find a PostgreSQL table" placeholder="Search tables and views" value={search} onChange={e => setSearch(e.target.value)} /></label>{!state.schema ? <p className="db-hint">Choose a schema.</p> : !state.relations.length ? <p className="db-hint">No accessible tables or views in this schema.</p> : !state.relations.some(r => r.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())) ? <p className="db-hint">No tables or views match “{search}”.</p> : state.relations.filter(r => r.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())).map(r => <button className={state.relation === r.name ? 'active' : ''} key={r.name} disabled={blocked} onClick={() => void controller.relation(r.name)}><Database size={13} /><span>{r.name}</span><small>{['v', 'm'].includes(r.kind) ? 'View' : 'Table'}</small></button>)}</div>}>{toggle => <div className="db-detail"><div className="db-detail-heading"><div className="db-detail-title">{toggle}<h2>{state.relation ? `${state.schema} / ${state.relation}` : 'PostgreSQL explorer'}</h2></div></div>
          <div className="pg-tabs" role="tablist" aria-label="PostgreSQL details" onKeyDown={event => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
            const tabs = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role=tab]:not(:disabled)')];
            const index = tabs.indexOf(document.activeElement as HTMLButtonElement); if (index < 0 || !tabs.length) return;
            event.preventDefault(); const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
            tabs[next].focus(); tabs[next].click();
          }}>{(Object.keys(paneNames) as Pane[]).filter(p => p !== 'view' || ['v', 'm'].includes(state.relations.find(r => r.name === state.relation)?.kind ?? '')).map(p => <button key={p} role="tab" tabIndex={state.pane === p || !state.relation && p === 'query' ? 0 : -1} aria-selected={state.pane === p} disabled={blocked || (p !== 'query' && !state.relation)} onClick={() => void controller.pane(p)}>{paneNames[p]}</button>)}</div>
          {state.pane === 'query' ? <div className="pg-editor">
            <div className="pg-editor-heading"><label htmlFor="pg-sql">SQL query</label><span className={`pg-editor-mode${mode === 'readWrite' ? ' pg-editor-mode-write' : ''}`}>{mode === 'readOnly' ? 'Read-only mode' : 'Changes require confirmation'}</span></div>
            <p className="pg-editor-context" id="pg-sql-context">Run a query against <strong>{connected.session.database}</strong>. Results appear below the editor.</p>
            <textarea id="pg-sql" aria-label="PostgreSQL SQL" aria-describedby="pg-sql-context pg-sql-mode-note" value={sql} onChange={e => setSql(e.target.value)} spellCheck={false} wrap="off" rows={10} disabled={!!state.pending} />
            <div className="pg-editor-toolbar"><label>Execution mode<select aria-label="SQL execution mode" value={mode} disabled={blocked} onChange={e => setMode(e.target.value as typeof mode)}><option value="readOnly">Read only (default)</option><option value="readWrite">Allow changes — confirmation required</option></select></label><button className="primary" disabled={blocked || !sql.trim() || new TextEncoder().encode(sql).length > 65536} onClick={() => { if (mode === 'readWrite') { setWrite({ sql, session: state.activeId }); setWriteConsent(false); } else void controller.sql(sql, false); }}>Run SQL</button></div>
            <p className="pg-editor-context" id="pg-sql-mode-note">{mode === 'readOnly' ? 'Use read-only mode to inspect your data. To change data, choose Allow changes and review the statement before running it.' : 'Run SQL opens a confirmation dialog. Review the exact statement and target database before authorizing changes.'}</p>
            <details className="pg-editor-notes"><summary>Query limits and safety notes</summary><ul><li>One statement at a time, up to 64 KiB of SQL.</li><li>Results show up to 200 rows. Statements time out after 10 seconds.</li><li>SQL text stays in memory for this app session.</li><li>Read-only transactions do not sandbox privileged functions. Cancellation does not guarantee rollback of every effect.</li></ul></details>
            {state.query && <div className="pg-editor-results"><h3>Query results</h3><PgResult output={state.query} title="SQL result" /></div>}
          </div> : state.relation ? <RowsAndDetails key={`${state.activeId}:${state.schema}:${state.relation}:${state.pane}`} controller={controller} blocked={blocked} /> : <div className="pg-empty"><Database size={28} /><h3>Choose a table or view</h3><p>Browse a schema, or use the SQL editor for an explicit query.</p>{!state.schemas.length && <p>No schemas available. Check privileges or refresh the explorer.</p>}</div>}
        </div>}</TableWorkspace></> : <div className="pg-empty"><Database size={28} /><h2>{state.connections.length ? 'Choose a connection' : 'No PostgreSQL connection'}</h2><p>Choose a saved connection above, or add your running local PostgreSQL server.</p></div>}
    </Workspace>
    <footer className="pg-browse-notes" aria-label="PostgreSQL browsing notes"><p><strong>Browsing notes</strong> Table rows are read-only here. Use the SQL editor for queries; modifying statements require your confirmation.</p><p>Saved connections remember your server details. Your external PostgreSQL server must already be running; connecting does not start or stop it.</p></footer>
    {connect && <ConnectionDialog platform={platform} managed={state.managed} initial={connect.initial} database={connect.database} profile={connect.profile} editing={connect.editing} busy={!!state.pending || profileBusy} error={dialogError} onClose={() => setConnect(null)} onConnect={submitConnection} onPrepareRuntime={prepareConnectionTools} />}
    {remove && <Dialog title="Remove saved connection?" busy={profileBusy} danger submit="Remove connection" onClose={() => setRemove(null)} onSubmit={() => void forgetProfile(remove)}><p>Forget <strong>{remove.name}</strong> and its saved password on this device. Your database, server and open sessions are kept.</p></Dialog>}
    {service && <Dialog title={serviceNames[service]} busy={!!state.pending} valid={serviceConsent && (service !== 'initialize' || Number.isInteger(Number(port)) && Number(port) >= 1024 && Number(port) <= 65535)} danger={service === 'initialize'} submit={serviceNames[service]} onClose={() => setService(null)} onSubmit={() => { const action = service; setService(null); void controller.service(action, Number(port)); }}><p>{service === 'provision' ? 'Verify and copy the pinned PostgreSQL runtime. This does not initialize or start a server.' : service === 'initialize' ? 'Create a new private SCRAM cluster and Keychain credential. Existing clusters are never adopted, overwritten or upgraded. If secure storage or runtime verification is unavailable, initialization fails closed.' : service === 'stop' || service === 'restart' ? 'Request smart shutdown of this Fairway-owned server. Active clients may delay it; there is no forced database shutdown. External installations are not controlled.' : 'Start only this verified Fairway-owned cluster. A running process does not prove authenticated readiness.'}</p>{service === 'initialize' && <label>Managed port<input aria-label="Managed PostgreSQL port" type="number" min={1024} max={65535} value={port} onChange={e => setPort(e.target.value)} /></label>}<label className="pg-check"><input type="checkbox" checked={serviceConsent} onChange={e => setServiceConsent(e.target.checked)} />Authorize this managed action</label></Dialog>}
    {write && <Dialog title="Authorize modifying SQL" busy={false} danger submit="Authorize and run once" valid={writeConsent && write.session === state.activeId && !blocked} onClose={() => setWrite(null)} onSubmit={() => { const statement = write.sql; setWrite(null); setMode('readOnly'); void controller.sql(statement, true); }}><p>Target: <strong>{connected?.source.kind === 'external' ? 'External server' : 'Managed cluster'} / {connected?.session.database}</strong></p><pre className="pg-sql-review">{write.sql}</pre><p>This statement may modify data. Limits and Cancel do not guarantee rollback. Nontransactional effects can remain; an uncertain outcome must be verified before retrying.</p><label className="pg-check"><input type="checkbox" checked={writeConsent} onChange={e => setWriteConsent(e.target.checked)} />I authorize this exact modifying statement on this database</label></Dialog>}
  </section>;
}
function RowsAndDetails({ controller, blocked }: { controller: PostgresWorkspace; blocked: boolean }) {
  const state = controller.snapshot();
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState('');
  const [direction, setDirection] = useState<'asc' | 'desc'>('asc');
  const [filter, setFilter] = useState('');
  const [operator, setOperator] = useState<NonNullable<PgPage['filters']>[number]['operator']>('contains');
  const [value, setValue] = useState('');
  // Metadata survives loading within this relation, never across a context switch.
  const columns = useRef(state.data?.columns.map(c => c.name) ?? []);
  if (state.data?.columns.length) columns.current = state.data.columns.map(c => c.name);
  const applied = useRef<PgPage>({ limit: 50 });
  const load = (next: number, apply = false) => {
    if (apply) applied.current = { limit: 50, sort: sort ? [{ column: sort, direction }] : [], filters: filter ? [{ column: filter, operator, value: ['isNull', 'notNull'].includes(operator) ? null : value }] : [] };
    setOffset(next); void controller.pane('rows', { ...applied.current, offset: next });
  };
  return <>{state.pane === 'rows' && <><div className="pg-row-controls"><label>Sort column<select aria-label="PostgreSQL sort column" disabled={blocked} value={sort} onChange={e => setSort(e.target.value)}><option value="">Primary key / server order</option>{columns.current.map(c => <option key={c}>{c}</option>)}</select></label><label>Direction<select aria-label="PostgreSQL sort direction" disabled={blocked} value={direction} onChange={e => setDirection(e.target.value as 'asc' | 'desc')}><option value="asc">Ascending</option><option value="desc">Descending</option></select></label><label>Filter column<select aria-label="PostgreSQL filter column" disabled={blocked} value={filter} onChange={e => setFilter(e.target.value)}><option value="">No filter</option>{columns.current.map(c => <option key={c}>{c}</option>)}</select></label><label>Operator<select aria-label="PostgreSQL filter operator" disabled={blocked || !filter} value={operator} onChange={e => setOperator(e.target.value as typeof operator)}>{(Object.keys(filterNames) as (keyof typeof filterNames)[]).map(o => <option key={o} value={o}>{filterNames[o]}</option>)}</select></label><label>Filter value<input aria-label="PostgreSQL filter value" disabled={blocked || !filter || ['isNull', 'notNull'].includes(operator)} value={value} maxLength={4096} onChange={e => setValue(e.target.value)} /></label><button className="secondary" disabled={blocked} onClick={() => load(0, true)}>Apply sort &amp; filter</button></div><div className="pg-actions pg-pagination"><button className="secondary" disabled={blocked || offset === 0} onClick={() => load(Math.max(0, offset - 50))}>Previous rows</button><span aria-live="polite">{state.pending ? 'Loading rows…' : state.data?.rows.length ? `Rows ${offset + 1}–${offset + state.data.rows.length}` : 'No rows'}<small>50 rows per page</small></span><button className="secondary" disabled={blocked || !state.data?.hasMore || offset >= 100000} onClick={() => load(offset + 50)}>Next rows</button></div></>}{state.data ? <PgResult output={state.data} title={paneNames[state.pane]} rowOffset={state.pane === 'rows' ? offset : 0} /> : !state.pending && <p className="pg-empty">No result. Refresh this tab to retry.</p>}</>;
}
