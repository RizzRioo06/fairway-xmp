import { Check, Clipboard, ExternalLink, FolderOpen, Lock, Pencil, Play, Plus, ScrollText, Terminal, Trash2 } from 'lucide-react';
import ActionMenu from './data/ActionMenu';
import { canControl } from './bridge';
import { hasPublicRoot, siteStatus, siteUrl } from './sites';
import type { Project, Snapshot } from './types';

type Props = {
  data: Snapshot; busy: boolean; copied: string | null;
  add: () => void; edit: (site: Project) => void; remove: (site: Project) => void;
  open: (target: string) => void; copy: (url: string, key: string) => void;
  terminal: (site: Project, parent: boolean) => void;
  start: () => void; logs: () => void; sync: () => void; retry: () => void;
  https: (site: Project, enabled: boolean) => void; removeTrust: () => void;
};

export default function SitesPage({ data, busy, copied, add, edit, remove, open, copy, terminal, start, logs, sync, retry, https, removeTrust }: Props) {
  const usable = canControl && data.runtimeReady && !busy && !data.sitesError;
  const server = data.services.find(service => service.id === 'nginx');
  return <div className="sites-page">
    <div className="page-heading"><div><h1>Your sites</h1><p>Local <code>.test</code> addresses and web roots.</p></div><button className="primary" disabled={!usable} onClick={add}><Plus size={15} />Add site</button></div>
    <section className="sites-server" aria-label="Site environment">
      <div><strong>{server?.ready ? 'Web server running' : server?.running ? 'Web server not ready' : 'Web server stopped'}</strong><p>All sites use PHP {data.phpVersion}. Files in <code>{data.documentRoot}</code> also open at <code>http://127.0.0.1:{data.ports.website}/</code>.</p></div>
      {!server?.ready && <button className="secondary" disabled={!canControl || (!server?.running && (!data.runtimeReady || busy))} onClick={server?.running ? logs : start}>{server?.running ? <ScrollText size={14} /> : <Play size={14} />}{server?.running ? 'View web server logs' : 'Start web server'}</button>}
    </section>
    {!data.runtimeReady && canControl && <div className="notice error" role="alert">The bundled web runtime is missing. Reinstall Fairway XMP to restore it. Your site files and saved registrations will be kept.</div>}
    {server?.message && <div className="notice error" role="alert">{server.message}</div>}
    {data.sitesError ? <div className="sites-load-error" role="alert"><h2>Your saved sites couldn’t be loaded</h2><p>Your site files haven’t been changed. Retry, or use these details when asking for help. Make a copy of the saved list before repairing it.</p><details><summary>Error details</summary><p>{data.sitesError}</p><p>Saved list: <code>{data.dataDir}{data.os === 'windows' ? '\\' : '/'}sites.json</code></p></details><button className="secondary" disabled={busy || !canControl} onClick={retry}>Retry loading sites</button></div> : <>
      {data.domainsPending && <div className="project-pending" role="status"><span>Finish address setup to open the marked sites. Your computer will ask for administrator permission.</span><button className="secondary" disabled={!usable} onClick={sync}>Set up addresses</button></div>}
      {data.projects.length === 0 ? <section className="sites-empty"><FolderOpen size={26} /><h2>No sites yet.</h2><p>Add an existing project folder to open it at an address such as <code>http://shop.test:{data.ports.website}/</code>. Adding a site keeps your files where they are.</p></section> : <ul className="sites-list" aria-label="Saved sites">{data.projects.map(site => {
        const status = siteStatus(site, data), ready = status === 'Ready to open';
        const url = siteUrl(site, data.ports.website);
        return <li key={site.domain} className="site-item" aria-label={site.name}>
          <div className="site-title"><h2>{site.name}</h2><span className={`site-status${ready ? ' ready' : ''}`}>{status}</span><ActionMenu className="site-action-menu" label={`Actions for ${site.name}`} disabled={busy || !canControl} actions={[
            { label: 'Open folder', icon: <FolderOpen size={15} />, onSelect: () => open(`site-folder:${site.domain}`), disabled: !site.available },
            { label: 'Terminal in web root', icon: <Terminal size={15} />, onSelect: () => terminal(site, false), disabled: !site.available || !data.runtimeReady },
            ...(hasPublicRoot(site.root) ? [{ label: 'Terminal in app folder', icon: <Terminal size={15} />, onSelect: () => terminal(site, true), disabled: !site.available || !data.runtimeReady }] : []),
            { label: 'Edit site', icon: <Pencil size={15} />, onSelect: () => edit(site), disabled: !usable },
            { label: 'Remove site', icon: <Trash2 size={15} />, onSelect: () => remove(site), disabled: !usable, danger: true, separatorBefore: true },
          ]} /></div>
          <div className="site-address-row"><code>{site.https && <Lock size={13} aria-label="HTTPS" />}{url}</code><div className="site-address-actions"><button className="secondary" aria-label={`Open ${site.name}`} disabled={!ready || busy || !canControl} onClick={() => open(`project:${site.domain}`)}><ExternalLink size={14} />Open</button><button className="icon-button" aria-label={`Copy URL for ${site.name}`} title="Copy URL" onClick={() => copy(url, site.domain)}>{copied === site.domain ? <Check size={16} /> : <Clipboard size={16} />}</button></div></div>
          <p className="site-root"><span>Web root</span><code>{site.root}</code></p>
          {!site.available && <p className="site-warning">The web root folder is missing. Edit this site to choose its new location.</p>}
          {status === 'HTTPS setup needed' && <button className="secondary" disabled={!usable} onClick={() => https(site, true)}>Set up HTTPS</button>}
          <div className="site-options"><label className="ssl-toggle"><input type="checkbox" role="switch" aria-label={`HTTPS for ${site.name}`} checked={site.https} disabled={!usable || (!site.https && (!site.available || !site.domainReady))} onChange={event => https(site, event.target.checked)} /><span className="ssl-track" aria-hidden="true" /><span>HTTPS</span></label><span>{site.indexFallback ? 'Missing paths route to index.php' : 'Serves files directly'}</span></div>
        </li>;
      })}</ul>}
      {data.httpsTrusted && !data.projects.some(site => site.https) && <div className="project-pending"><span>HTTPS is off for all sites. Fairway’s local certificate is still trusted by this computer.</span><button className="secondary" disabled={!usable} onClick={removeTrust}>Remove certificate trust</button></div>}
    </>}
  </div>;
}
