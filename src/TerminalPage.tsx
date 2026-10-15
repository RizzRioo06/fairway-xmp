import { useEffect, useRef, useState } from 'react';
import { Check, Clipboard, Copy, Eraser, RefreshCw, Square, Terminal as TerminalIcon } from 'lucide-react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { canControl, invoke, isUiTest } from './bridge';
import '@xterm/xterm/css/xterm.css';
import './terminal.css';

type Info = { shell: string; directory: string };
type Output = { data: number[]; exited: boolean; exitCode: number | null; error: string | null };
type Status = 'starting' | 'running' | 'ended' | 'error';
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

export default function TerminalPage({ active, busy }: { active: boolean; busy: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const session = useRef<string | null>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<Status>('starting');
  const [info, setInfo] = useState<Info | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hasSelection, setHasSelection] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirm, setConfirm] = useState<'restart' | 'stop' | null>(null);

  useEffect(() => {
    if (!host.current || !canControl) return;
    let disposed = false;
    let timer: number | undefined;
    let frame = 0;
    let failed = false;
    let queuedBytes = 0;
    let writing = Promise.resolve();
    const id = crypto.randomUUID();
    const decoder = new TextDecoder();
    const term = new Terminal({
      cursorBlink: true, fontSize: 13, lineHeight: 1.25, scrollback: 3000,
      fontFamily: 'Menlo, Consolas, "Liberation Mono", monospace', screenReaderMode: true,
      theme: { background: '#17251f', foreground: '#e0ebe4', cursor: '#a3d9bc', selectionBackground: '#52796280',
        black: '#263b30', red: '#ec9993', green: '#a0d5af', yellow: '#e6ce92', blue: '#9dbfe6', magenta: '#c4a9d9', cyan: '#92d0cc', white: '#e0ebe4',
        brightBlack: '#8b9e91', brightRed: '#f5b1aa', brightGreen: '#b8e8c5', brightYellow: '#f4dfa9', brightBlue: '#b9d6f5', brightMagenta: '#dac0ef', brightCyan: '#b3e5e0', brightWhite: '#ffffff' },
    });
    const addon = new FitAddon();
    term.loadAddon(addon); term.open(host.current);
    term.textarea?.setAttribute('aria-label', 'Terminal input');
    terminal.current = term; fit.current = addon;
    setStatus('starting'); setError(null); setInfo(null); setHasSelection(false);

    const fail = (reason: unknown) => {
      if (disposed || failed) return;
      failed = true; setError(errorText(reason)); setStatus('error');
      term.options.disableStdin = true; session.current = null;
      void invoke('terminal_close', { id }).catch(() => {});
    };
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        if (!disposed && host.current?.clientWidth && host.current.clientHeight) addon.fit();
      });
    });
    observer.observe(host.current);
    const resize = term.onResize(({ cols, rows }) => {
      if (session.current === id) void invoke('terminal_resize', { id, cols, rows }).catch(reason => { if (session.current === id) fail(reason); });
    });
    const selection = term.onSelectionChange(() => { if (!disposed) setHasSelection(term.hasSelection()); });
    const input = term.onData(data => {
      if (session.current !== id || failed) return;
      // Serialize keys and chunk pastes; never queue unlimited IPC writes.
      if (queuedBytes + data.length > 256 * 1024) { setError('Paste is too large. Use a smaller selection.'); return; }
      queuedBytes += data.length;
      writing = writing.then(async () => {
        for (let offset = 0; offset < data.length;) {
          let end = Math.min(offset + 1024, data.length);
          if (end < data.length && /[\uD800-\uDBFF]/.test(data[end - 1])) end--;
          if (disposed || failed || session.current !== id) break;
          await invoke('terminal_write', { id, data: data.slice(offset, end) });
          offset = end;
        }
      }).catch(reason => { if (session.current === id) fail(reason); }).finally(() => { queuedBytes -= data.length; });
    });
    term.attachCustomKeyEventHandler(event => {
      if ((event.metaKey || (event.ctrlKey && event.shiftKey)) && ['c', 'v'].includes(event.key.toLowerCase())) return false;
      if (event.ctrlKey && !event.shiftKey && event.key.toLowerCase() === 'c' && term.hasSelection()) return false;
      return true;
    });
    const poll = async () => {
      if (disposed || failed || session.current !== id) return;
      try {
        const output = await invoke<Output>('terminal_read', { id });
        if (disposed || session.current !== id) return;
        const text = decoder.decode(new Uint8Array(output.data), { stream: !output.exited });
        // Drain more only after xterm has parsed this batch, bounding its queue.
        if (text) await new Promise<void>(resolve => term.write(text, resolve));
        if (disposed || session.current !== id) return;
        if (output.error) { fail(output.error); return; }
        if (output.exited) {
          term.writeln(`\r\n[Session ended${output.exitCode !== null ? ` · exit ${output.exitCode}` : ''}]`);
          term.options.disableStdin = true; session.current = null; setStatus('ended');
          void invoke('terminal_close', { id }).catch(() => {});
        } else timer = window.setTimeout(() => { void poll(); }, output.data.length ? 10 : 70);
      } catch (reason) { if (session.current === id) fail(reason); }
    };
    // StrictMode's initial cleanup runs before this opens a native shell.
    void Promise.resolve().then(async () => {
      if (disposed) return;
      try {
        addon.fit();
        const result = await invoke<Info>('terminal_start', { id, cols: Math.max(2, term.cols), rows: Math.max(1, term.rows) });
        if (disposed) { await invoke('terminal_close', { id }); return; }
        session.current = id; setInfo(result); setStatus('running');
        addon.fit(); term.focus(); void poll();
      } catch (reason) { fail(reason); }
    });
    return () => {
      disposed = true; clearTimeout(timer); cancelAnimationFrame(frame); observer.disconnect();
      resize.dispose(); selection.dispose(); input.dispose(); term.dispose();
      terminal.current = null; fit.current = null;
      if (session.current === id) session.current = null;
      void invoke('terminal_close', { id }).catch(() => {});
    };
  }, [generation]);

  useEffect(() => {
    if (active) {
      const frame = requestAnimationFrame(() => { fit.current?.fit(); terminal.current?.focus(); });
      return () => cancelAnimationFrame(frame);
    }
  }, [active]);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(timer);
  }, [copied]);

  useEffect(() => {
    if (!confirm) return;
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.querySelector('button')?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); setConfirm(null); }
      if (event.key !== 'Tab') return;
      const buttons = dialog.current?.querySelectorAll<HTMLButtonElement>('button');
      if (!buttons?.length) return;
      const first = buttons[0], last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('keydown', key); previous?.focus(); };
  }, [confirm]);

  async function clipboard(action: 'copy' | 'paste') {
    try {
      if (action === 'copy') { await navigator.clipboard.writeText(terminal.current?.getSelection() ?? ''); setCopied(true); }
      else { const text = await navigator.clipboard.readText(); terminal.current?.paste(text); terminal.current?.focus(); }
    } catch { setError(`Could not ${action}. Use your terminal’s keyboard shortcut or clipboard permissions.`); }
  }

  async function endSession(restart: boolean) {
    setConfirm(null); setError(null);
    const id = session.current; session.current = null;
    if (terminal.current) terminal.current.options.disableStdin = true;
    setStatus(restart ? 'starting' : 'ended');
    try {
      if (id) await invoke('terminal_close', { id });
      if (restart) setGeneration(value => value + 1);
      else terminal.current?.writeln('\r\n[Session ended]');
    } catch (reason) { setError(errorText(reason)); setStatus('error'); }
  }

  return <section className="terminal-page" aria-label="Embedded terminal">
    <div className="page-heading"><div><h1>Terminal</h1><p>PHP and Composer, ready in your coursework folder.</p></div><span className={`status-label ${status === 'running' ? 'ready' : ''}`} role="status"><span className={`status-dot ${status === 'running' ? 'running' : ''}`} />{canControl ? { starting: 'Starting…', running: 'Connected', ended: 'Session ended', error: 'Disconnected' }[status] : 'Desktop app required'}</span></div>
    {isUiTest && <div className="preview-banner">UI test — simulated terminal. Commands do not run on your computer.</div>}
    {error && <div className="notice error" role="alert">{error}</div>}
    <div className="terminal-panel">
      <div className="terminal-toolbar"><span className="terminal-shell"><TerminalIcon size={16} />{info?.shell ?? 'Local shell'}</span><div className="terminal-actions">
        <button className="secondary" disabled={!canControl} onClick={() => { terminal.current?.clear(); terminal.current?.focus(); }}><Eraser size={14} />Clear</button>
        <button className="secondary" disabled={!hasSelection} onClick={() => { void clipboard('copy'); }}>{copied ? <Check size={14} /> : <Copy size={14} />}Copy</button>
        <button className="secondary" disabled={status !== 'running' || !canControl} onClick={() => { void clipboard('paste'); }}><Clipboard size={14} />Paste</button>
        <button className="secondary" disabled={!canControl || status === 'starting' || busy} onClick={() => { if (status === 'running') setConfirm('restart'); else void endSession(true); }}><RefreshCw size={14} />{status === 'running' ? 'Restart' : 'New session'}</button>
        <button className="secondary" disabled={status !== 'running' || !canControl} onClick={() => setConfirm('stop')}><Square size={13} />End session</button>
      </div></div>
      {canControl ? <div ref={host} className="terminal-host" /> : <p className="terminal-unavailable">Open the Fairway XMP desktop app to use the terminal.</p>}
      <div className="terminal-directory"><span>Started in</span><code title={info?.directory}>{info?.directory ?? 'Your coursework folder'}</code></div>
    </div>
    <p className="terminal-hint">Ctrl+C interrupts a command. Switching pages or closing the window keeps this session running. Quitting Fairway XMP ends it.</p>
    {confirm && <div className="modal-backdrop"><div ref={dialog} className="modal terminal-confirm" role="dialog" aria-modal="true" aria-labelledby="terminal-confirm-title"><h2 id="terminal-confirm-title">{confirm === 'restart' ? 'Restart terminal?' : 'End terminal session?'}</h2><p>This ends the shell and its running commands. {confirm === 'restart' ? 'A new shell will open in your coursework folder.' : 'You can start a new session afterward.'}</p><div className="button-row"><button className="secondary" onClick={() => setConfirm(null)}>Cancel</button><button className="primary" onClick={() => { void endSession(confirm === 'restart'); }}>{confirm === 'restart' ? 'Restart terminal' : 'End session'}</button></div></div></div>}
  </section>;
}
