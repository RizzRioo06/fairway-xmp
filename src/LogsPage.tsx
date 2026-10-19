import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Check, Clipboard, Pause, Play, RefreshCw, ScrollText, Search } from 'lucide-react';
import { canControl, invoke } from './bridge';
import type { LogEntry, LogSnapshot, Project, ServiceStatus } from './types';
import './logs.css';

const sources: Record<string, string> = { php: 'PHP', nginx: 'Web server', requests: 'Requests', mysql: 'MySQL', mail: 'Mail' };
const dateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'medium' });
const MAX_VISIBLE = 240;
export type LogRequest = { key: number; source: string };
const empty: LogSnapshot = { entries: [], unavailable: [], limited: false };

function sourceName(entry: LogEntry) { return entry.source === 'php' ? 'Shared PHP (all sites)' : sources[entry.source]; }

export default function LogsPage({ active, services, projects, request }: { active: boolean; services: ServiceStatus[]; projects: Project[]; request: LogRequest | null }) {
  const [snapshot, setSnapshot] = useState<LogSnapshot>(empty);
  const [source, setSource] = useState('all');
  const [site, setSite] = useState('');
  const [query, setQuery] = useState('');
  const [problems, setProblems] = useState(true);
  const [live, setLive] = useState(true);
  const refreshSeen = useRef(0);
  const revision = useRef<string | null>(null);
  const pendingRead = useRef<Promise<LogSnapshot> | null>(null);
  const loadedRef = useRef(false);
  const servicesRef = useRef(services);
  servicesRef.current = services;
  const [shownServices, setShownServices] = useState<ServiceStatus[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const activeRef = useRef(active);
  activeRef.current = active;
  const operationFocus = useRef<HTMLElement | null>(null);
  const [settled, setSettled] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [visible, setVisible] = useState(!document.hidden);
  const scrolling = useRef<HTMLDivElement>(null);
  const position = useRef<{ bottom: boolean; id?: string; top: number }>({ bottom: true, top: 0 });
  const copyingTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    const visibility = () => setVisible(!document.hidden);
    document.addEventListener('visibilitychange', visibility);
    return () => { document.removeEventListener('visibilitychange', visibility); window.clearTimeout(copyingTimer.current); };
  }, []);
  useEffect(() => { if (!active) setCopyError(null); }, [active]);
  useEffect(() => {
    if (!operationFocus.current) return;
    const trigger = operationFocus.current;
    operationFocus.current = null;
    // Copy does not disable its button, but a failed write can still blur it.
    // Restore that control without scrolling, and never take focus back after
    // navigation or from an open dialog.
    if (activeRef.current && document.activeElement === document.body && !document.querySelector('[aria-modal="true"]') && trigger.isConnected && !trigger.matches(':disabled')) {
      trigger.focus({ preventScroll: true });
    }
  }, [settled]);

  useEffect(() => {
    if (!request) return;
    position.current.bottom = true;
    setSource(request.source); setSite(''); setQuery(''); setLive(true);
    setRefresh(value => value + 1);
  }, [request]);

  // Serial bounded snapshots, only while this page and the app window are visible.
  // Native log reads use a worker independent of the service-operation lock.
  useEffect(() => {
    if (!active || !visible || !canControl || (!live && loadedRef.current && refresh === refreshSeen.current)) return;
    refreshSeen.current = refresh;
    let disposed = false;
    let timer: number | undefined;
    const load = async () => {
      try {
        // Reuse an outstanding read across visibility, pause and shortcut effects.
        const pending = pendingRead.current ?? invoke<LogSnapshot>('read_logs');
        pendingRead.current = pending;
        let next: LogSnapshot;
        try { next = await pending; }
        finally { if (pendingRead.current === pending) pendingRead.current = null; }
        if (!disposed) {
          const nextRevision = next.revision ?? JSON.stringify(next);
          if (revision.current !== nextRevision) { setSnapshot(next); revision.current = nextRevision; }
          setShownServices(previous => previous.length === servicesRef.current.length && previous.every((service, index) => service.id === servicesRef.current[index].id && service.message === servicesRef.current[index].message) ? previous : servicesRef.current);
          setError(null); setLoaded(true); loadedRef.current = true;
        }
      } catch (reason) { if (!disposed) setError(String(reason)); }
      if (!disposed && live) timer = window.setTimeout(() => { void load(); }, 1500);
    };
    void load();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [active, visible, live, refresh]);

  const siteOptions = useMemo(() => {
    const hosts = new Map(projects.map(project => [`${project.domain}.test`, `${project.name} (${project.domain}.test)`]));
    for (const entry of snapshot.entries) {
      if (entry.source === 'requests' && entry.host && !hosts.has(entry.host)) hosts.set(entry.host, entry.host);
    }
    // Keep the selected address available after a rename or when it ages out of history.
    if (site && !hosts.has(site)) hosts.set(site, site);
    return [...hosts].sort(([a], [b]) => a.localeCompare(b));
  }, [projects, snapshot, site]);
  const unscopedRequests = snapshot.entries.some(entry => entry.source === 'requests' && !entry.host);

  const matching = useMemo(() => {
    const current: LogEntry[] = shownServices.filter(service => service.message).map(service => ({
      id: `current:${service.id}`, source: service.id === 'phpmyadmin' ? 'nginx' : service.id,
      file: 'Current service status', severity: 'error', time: null, text: service.message!.length > 4000 ? `${service.message!.slice(0, 4000)}\n… Message shortened.` : service.message!,
    }));
    const needle = query.trim().toLowerCase();
    return [...snapshot.entries, ...current].filter(entry =>
      (!site || entry.source === 'php' || (entry.source === 'requests' && entry.host === site)) &&
      (source === 'all' || entry.source === source) && (!problems || entry.severity !== 'info') &&
      (!needle || `${sourceName(entry)} ${entry.file} ${entry.text}`.toLowerCase().includes(needle)),
    ).sort((a, b) => {
      const aTime = a.time ? Date.parse(a.time) : NaN;
      const bTime = b.time ? Date.parse(b.time) : NaN;
      // Undated output stays in source order after dated entries; do not invent times.
      return (Number.isFinite(aTime) ? aTime : Number.MAX_SAFE_INTEGER) - (Number.isFinite(bTime) ? bTime : Number.MAX_SAFE_INTEGER);
    });
  }, [snapshot, shownServices, source, site, query, problems]);

  const entries = useMemo(() => {
    if (matching.length <= MAX_VISIBLE) return matching;
    // Keep real failures from being crowded out by undated routine output.
    const priority: Record<string, number> = { error: 0, warning: 1, unknown: 2, info: 3 };
    const selected = new Set(matching.map((entry, index) => ({ entry, index }))
      .sort((a, b) => (priority[a.entry.severity] ?? 2) - (priority[b.entry.severity] ?? 2) || b.index - a.index)
      .slice(0, MAX_VISIBLE).map(item => item.index));
    return matching.filter((_, index) => selected.has(index));
  }, [matching]);
  const hasHistory = snapshot.entries.length > 0 || shownServices.some(service => service.message);
  const emptyTitle = !hasHistory ? snapshot.unavailable.length ? 'Logs unavailable' : 'No log entries yet'
    : query.trim() || site || source !== 'all' ? 'No matching entries' : 'No problems recorded';
  const emptyHelp = !hasHistory ? snapshot.unavailable.length ? 'Resolve the read errors above, then try again.' : 'Start a service or open a local site, then check here again.'
    : problems ? 'Try changing the filters or turn off Problems only to see other activity.' : 'Try changing the site, source or search.';

  function rememberPosition() {
    const element = scrolling.current;
    if (!element) return;
    const top = element.getBoundingClientRect().top;
    const anchor = Array.from(element.querySelectorAll<HTMLElement>('[data-entry]')).find(row => row.getBoundingClientRect().bottom > top);
    position.current = { bottom: element.scrollHeight - element.scrollTop - element.clientHeight < 28, id: anchor?.dataset.entry, top: (anchor?.getBoundingClientRect().top ?? top) - top };
  }
  useLayoutEffect(() => {
    const element = scrolling.current;
    if (!element) return;
    if (position.current.bottom) element.scrollTop = element.scrollHeight;
    else {
      const anchor = Array.from(element.querySelectorAll<HTMLElement>('[data-entry]')).find(row => row.dataset.entry === position.current.id);
      if (anchor) element.scrollTop += anchor.getBoundingClientRect().top - element.getBoundingClientRect().top - position.current.top;
    }
    rememberPosition();
  }, [entries]);

  function changeFilter(action: () => void) { position.current.bottom = true; action(); }
  async function copy() {
    operationFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    try {
      await navigator.clipboard.writeText(entries.map(entry => `[${entry.time ?? 'Time unavailable'}] ${sourceName(entry)} · ${entry.file}\n${entry.text}`).join('\n\n'));
      if (!activeRef.current) return;
      setCopyError(null); setCopied(true); window.clearTimeout(copyingTimer.current);
      copyingTimer.current = window.setTimeout(() => setCopied(false), 1800);
    } catch (reason) {
      if (!activeRef.current) return;
      setCopied(false);
      const extra = reason instanceof Error ? reason.message.trim() : '';
      const sentence = extra ? (/[.!?]$/.test(extra) ? extra : `${extra}.`) : '';
      setCopyError(sentence ? `Could not copy logs. ${sentence} Select the text and copy it manually, or retry.` : 'Could not copy logs. Select the text and copy it manually, or retry.');
    } finally { setSettled(value => value + 1); }
  }

  return <div className={`logs-page${site ? ' logs-site-active' : ''}`}>
    <div className={`logs-notice${copyError ? ' notice error' : ''}`}>
      <div className="logs-notice-message">
        <div role={copyError ? 'alert' : undefined} aria-live="assertive" aria-atomic="true">{copyError}</div>
      </div>
      {copyError && <button onClick={() => { void copy(); }}>Retry copy</button>}
      {copyError && <button aria-label="Dismiss logs message" onClick={() => setCopyError(null)}>×</button>}
    </div>
    <div className="page-heading"><div><h1>Logs</h1><p>Check here when something goes wrong.</p></div></div>
    <section className="logs-workspace" aria-label="Environment logs">
      <div className="logs-toolbar">
        <label className="logs-source logs-site">Site<select aria-label="Log site" value={site} onChange={event => changeFilter(() => {
          const next = event.target.value; setSite(next);
          if (next && !['all', 'requests', 'php'].includes(source)) setSource('all');
        })}><option value="">All sites</option>{siteOptions.map(([host, label]) => <option key={host} value={host}>{label}</option>)}</select></label>
        <label className="logs-source">Source<select aria-label="Log source" value={source} onChange={event => changeFilter(() => setSource(event.target.value))}><option value="all">All sources</option>{Object.entries(sources).filter(([value]) => !site || value === 'requests' || value === 'php').map(([value, label]) => <option key={value} value={value}>{value === 'php' ? 'Shared PHP' : label}</option>)}</select></label>
        <label className="logs-search"><Search size={15} /><input type="search" aria-label="Search logs" placeholder="Search logs" value={query} onChange={event => changeFilter(() => setQuery(event.target.value))} /></label>
        <label className="logs-problems"><input type="checkbox" checked={problems} onChange={event => changeFilter(() => setProblems(event.target.checked))} />Problems only</label>
      </div>
      {site && <p className="logs-scope">Requests for <strong>{site}</strong>. PHP entries are shared across all sites; they may belong to another site. Other service logs are under All sites.</p>}
      <div className="logs-actions"><span role="status">{loaded ? `${entries.length}${matching.length > MAX_VISIBLE ? ` of ${matching.length}` : ''} ${entries.length === 1 ? 'entry' : 'entries'} · ${live ? 'Live' : 'Paused'}` : canControl ? 'Loading logs…' : 'Open the desktop app to read logs.'}</span><div>
        <button className="secondary" disabled={!canControl} onClick={() => setLive(value => !value)}>{live ? <Pause size={14} /> : <Play size={14} />}{live ? 'Pause' : 'Resume'}</button>
        {!live && <button className="secondary" disabled={!canControl} onClick={() => setRefresh(value => value + 1)}><RefreshCw size={14} />Refresh</button>}
        <button className="secondary" disabled={!entries.length} onClick={() => { void copy(); }}>{copied ? <Check size={14} /> : <Clipboard size={14} />}{copied ? 'Copied' : 'Copy shown'}</button>
      </div></div>
      {error && <div className="logs-error" role="alert">{error} <button className="text-button" onClick={() => setRefresh(value => value + 1)}>Retry</button></div>}
      {!!snapshot.unavailable.length && <div className="logs-error" role="alert"><strong>Some logs could not be read.</strong>{snapshot.unavailable.map(item => <p key={item}>{item}</p>)}</div>}
      <div className="logs-output" ref={scrolling} onScroll={rememberPosition} tabIndex={0} role="region" aria-label="Log entries">
        {entries.map(entry => <article className={`log-entry log-${entry.severity}`} key={entry.id} data-entry={entry.id}>
          <div className="log-entry-meta"><strong>{entry.source === 'php' ? 'Shared PHP' : sources[entry.source]}</strong>{entry.source === 'php' && <span>All sites</span>}<span>{entry.severity === 'unknown' ? 'Unclassified' : entry.severity}</span><span>{entry.file}</span>{entry.time && <time dateTime={entry.time}>{Number.isNaN(Date.parse(entry.time)) ? entry.time : dateFormatter.format(new Date(entry.time))}</time>}</div>
          <pre>{entry.text}</pre>
        </article>)}
        {loaded && !entries.length && <div className="logs-empty"><ScrollText size={25} /><h2>{emptyTitle}</h2><p>{emptyHelp}</p></div>}
      </div>
      <p className="logs-hint">{matching.length > MAX_VISIBLE && <>Showing up to {MAX_VISIBLE} matches, prioritizing errors and warnings, then recent output. Narrow the source or search to find other entries. </>}{site ? 'Recent requests for the selected address and shared PHP output.' : 'Recent output from all sites and services. PHP errors are shared.'}{site && unscopedRequests && ' Earlier requests without a recorded host remain under All sites.'}  Problems only includes warnings, failed requests, and unclassified output.{snapshot.limited ? ' Older entries are outside this recent view.' : ''}</p>
    </section>
  </div>;
}
