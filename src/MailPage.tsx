import { useCallback, useEffect, useState } from 'react';
import { Download, ExternalLink, Inbox, Mail, Paperclip, Play, ScrollText, Search, Settings2, Square, Trash2 } from 'lucide-react';
import { canControl, invoke } from './bridge';
import type { MailAddress, MailAttachment, MailList, MailMessage, Snapshot } from './types';

type Props = {
  data: Snapshot; busy: boolean;
  start: () => void; stop: () => void; logs: () => void;
  open: () => void; settings: () => void;
};

function message(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function address(item: MailAddress | null | undefined) {
  return item ? item.Name || item.Address : 'Unknown sender';
}

function addresses(items: MailAddress[] | null) {
  return items?.length ? items.map(item => item.Name ? `${item.Name} <${item.Address}>` : item.Address).join(', ') : '';
}

function when(value: string) {
  const date = new Date(value);
  return date.toDateString() === new Date().toDateString()
    ? date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    : date.toLocaleDateString([], { day: 'numeric', month: 'short' });
}

// A readable default for unstyled emails; an email's own CSS still wins.
const emailBase = '<style>body{margin:16px 20px;font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:#22342a;overflow-wrap:anywhere}img{max-width:100%;height:auto}</style>';

function size(bytes: number) {
  return bytes < 1024 ? `${bytes} B` : bytes < 1048576 ? `${Math.round(bytes / 1024)} KB` : `${(bytes / 1048576).toFixed(1)} MB`;
}

export default function MailPage({ data, busy, start, stop, logs, open, settings }: Props) {
  const mail = data.services.find(service => service.id === 'mail');
  const ready = !!mail?.ready;
  const usable = canControl && data.runtimeReady && !busy;
  const [query, setQuery] = useState('');
  const [list, setList] = useState<MailList | null>(null);
  const [selected, setSelected] = useState<MailMessage | null>(null);
  const [tab, setTab] = useState<'html' | 'text'>('html');
  const [confirmClear, setConfirmClear] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  const load = useCallback(async (search: string) => {
    try {
      setList(await invoke<MailList>('mail_inbox', { request: { action: 'list', query: search } }));
      setError(null);
    } catch (reason) { setError(message(reason)); }
  }, []);

  // New mail shows up without a manual refresh while the inbox is open.
  useEffect(() => {
    if (!ready) { setList(null); setSelected(null); return; }
    const timer = window.setTimeout(() => { void load(query); }, query ? 300 : 0);
    const poll = window.setInterval(() => { void load(query); }, 3000);
    return () => { window.clearTimeout(timer); window.clearInterval(poll); };
  }, [ready, query, load]);

  // Close the preview when its email was deleted elsewhere, such as in the browser inbox.
  useEffect(() => {
    if (selected && list && !query && !list.messages.some(item => item.ID === selected.ID)) setSelected(null);
  }, [list, selected, query]);

  async function show(id: string) {
    try {
      const opened = await invoke<MailMessage>('mail_inbox', { request: { action: 'message', id } });
      setSelected(opened); setTab(opened.HTML ? 'html' : 'text'); setSaved(null);
      setList(current => current && { ...current, messages: current.messages.map(item => item.ID === id ? { ...item, Read: true } : item) });
    } catch (reason) { setError(message(reason)); }
  }

  async function remove(request: Record<string, unknown>) {
    try {
      await invoke('mail_inbox', { request });
      setSelected(null); setConfirmClear(false);
      await load(query);
    } catch (reason) { setError(message(reason)); }
  }

  async function save(id: string, item: MailAttachment) {
    try {
      const path = await invoke<string | null>('save_mail_attachment', { id, part: item.PartID, name: item.FileName || 'attachment' });
      if (path) setSaved(`Saved to ${path}`);
    } catch (reason) { setError(message(reason)); }
  }

  const unread = list?.messages_unread ?? 0;
  return <div className="mail-page">
    <div className="page-heading"><div><h1>Mail</h1><p>Catches emails from your apps. Nothing reaches real inboxes.</p></div>
      <div className="mail-heading-actions"><span className={`status-label ${ready ? 'ready' : mail?.running ? 'pending' : ''}`}><span className={`status-dot ${ready ? 'running' : mail?.running ? 'pending' : ''}`} />{ready ? 'Running' : mail?.running ? 'Waiting' : 'Stopped'}</span>
        <button className="secondary" onClick={settings}><Settings2 size={14} />SMTP settings</button>
        {mail?.running ? <button className="primary" disabled={!canControl || busy} onClick={stop}><Square size={14} />Stop mail</button> : <button className="primary" disabled={!usable} onClick={start}><Play size={14} fill="currentColor" />Start mail</button>}</div>
    </div>
    {mail?.message && <div className="notice error" role="alert"><span>{mail.message}</span></div>}
    {!ready ? <section className="mail-inbox mail-empty"><Mail size={26} /><h2>{mail?.running ? 'Mail is starting…' : 'Mail is stopped.'}</h2><p>Start mail to catch the emails your apps send. <strong>Start all</strong> on Environment also starts it.</p>{!mail?.running && <button className="primary" disabled={!usable} onClick={start}><Play size={14} fill="currentColor" />Start mail</button>}</section> :
      <section className="mail-inbox mail-workspace" aria-label="Mail inbox">
        <div className="mail-list-pane">
          <label className="mail-search"><Search size={14} /><input type="search" aria-label="Search emails" placeholder="Search emails" value={query} onChange={event => setQuery(event.target.value)} /></label>
          <div className="mail-list-summary"><span>{list ? `${list.messages_count} ${list.messages_count === 1 ? 'email' : 'emails'}${unread ? ` · ${unread} unread` : ''}` : 'Loading…'}</span>
            {confirmClear ? <span className="mail-confirm"><button className="text-button" onClick={() => setConfirmClear(false)}>Cancel</button><button className="text-button danger" onClick={() => { void remove({ action: 'clear' }); }}>Delete all</button></span>
              : <button className="text-button" disabled={!list?.messages.length} onClick={() => setConfirmClear(true)}><Trash2 size={13} />Delete all</button>}</div>
          {error && <p className="mail-error" role="alert">{error}</p>}
          {list && list.messages.length === 0 ? <div className="mail-list-empty"><Inbox size={22} /><p>{query ? 'No emails match your search.' : 'No emails yet. Emails your apps send appear here.'}</p></div> :
            <ul className="mail-list" aria-label="Emails">{list?.messages.map(item => <li key={item.ID}>
              <button className={`${item.ID === selected?.ID ? 'active ' : ''}${item.Read ? '' : 'unread'}`} aria-current={item.ID === selected?.ID} onClick={() => { void show(item.ID); }}>
                <span className="mail-item-top"><span className="mail-from">{!item.Read && <span className="mail-unread-dot" aria-label="Unread" />}{address(item.From)}</span><time>{when(item.Created)}</time></span>
                <span className="mail-subject">{item.Subject || '(No subject)'}{item.Attachments > 0 && <Paperclip size={12} aria-label="Has attachments" />}</span>
                <span className="mail-snippet">{item.Snippet}</span>
              </button></li>)}</ul>}
        </div>
        <div className="mail-preview">{selected ? <>
          <div className="mail-preview-head">
            <div className="mail-preview-title"><h2>{selected.Subject || '(No subject)'}</h2><button className="icon-button" title="Delete email" aria-label="Delete email" onClick={() => { void remove({ action: 'delete', ids: [selected.ID] }); }}><Trash2 size={15} /></button></div>
            <dl className="mail-meta"><div><dt>From</dt><dd>{addresses(selected.From ? [selected.From] : null)}</dd></div><div><dt>To</dt><dd>{addresses(selected.To)}</dd></div>{selected.Cc?.length ? <div><dt>Cc</dt><dd>{addresses(selected.Cc)}</dd></div> : null}<div><dt>Date</dt><dd>{new Date(selected.Date).toLocaleString()}</dd></div></dl>
            {selected.Attachments?.length ? <ul className="mail-attachments" aria-label="Attachments">{selected.Attachments.map(item => <li key={item.PartID}><button title="Save attachment" disabled={!canControl} onClick={() => { void save(selected.ID, item); }}><Download size={12} />{item.FileName || 'Attachment'}<span>{size(item.Size)}</span></button></li>)}</ul> : null}
            {saved && <p className="mail-saved" role="status">{saved}</p>}
            <div className="mail-tabs" role="tablist">{selected.HTML && <button role="tab" aria-selected={tab === 'html'} onClick={() => setTab('html')}>HTML</button>}<button role="tab" aria-selected={tab === 'text'} onClick={() => setTab('text')}>Text</button></div>
          </div>
          {/* Scripts, forms, and navigation stay disabled; email HTML is untrusted. */}
          {tab === 'html' && selected.HTML ? <iframe className="mail-body" title="Email content" sandbox="" srcDoc={emailBase + selected.HTML} /> : <pre className="mail-body mail-text">{selected.Text || 'This email has no text version.'}</pre>}
        </> : <div className="mail-list-empty"><Mail size={22} /><p>Select an email to read it.</p></div>}</div>
      </section>}
    <div className="mail-footer">
      <button className="secondary" disabled={!canControl} onClick={logs}><ScrollText size={14} />View logs</button>
      <button className="secondary" disabled={!ready || busy || !canControl} onClick={open}><ExternalLink size={14} />Open in browser</button></div>
  </div>;
}
