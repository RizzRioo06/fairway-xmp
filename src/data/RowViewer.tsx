import { useEffect, useId, useRef, useState } from 'react';
import { Check, Copy, Pencil, X } from 'lucide-react';
import { useModalFocus } from '../modal-focus';
import type { Column, Row } from './types';

export default function RowViewer({ table, columns, row, position, onClose, onEdit }: {
  table: string; columns: Column[]; row: Row; position?: number; onClose: () => void; onEdit?: () => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const title = useId();
  const context = useId();
  const copying = useRef(0);
  const [copied, setCopied] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{ error: boolean; text: string } | null>(null);
  useModalFocus(container, true, onClose);
  useEffect(() => () => { copying.current++; }, []);

  async function copy(key: string, text: string, label: string) {
    const request = ++copying.current;
    try {
      await navigator.clipboard.writeText(text);
      if (request !== copying.current) return;
      setCopied(key); setFeedback({ error: false, text: `${label} copied.` });
    } catch {
      if (request !== copying.current) return;
      setCopied(null); setFeedback({ error: true, text: 'Could not copy. Select the value and copy it manually, or try again.' });
    }
  }

  return <div className="modal-backdrop"><div ref={container} className="modal db-row-viewer" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={context}>
    <div className="modal-form">
      <header className="card-title"><h2 id={title}>View row</h2><button className="icon-button" type="button" aria-label="Close row viewer" onClick={onClose}><X size={18} /></button></header>
      <div className="db-row-viewer-context" id={context}><strong>{table}</strong><span>{position ? `Row ${position} · ` : ''}{columns.length} {columns.length === 1 ? 'field' : 'fields'}</span></div>
      <div className="modal-form-body">
        <dl className="db-row-values">{columns.map(column => {
          const value = row[column.name] ?? null;
          return <div key={column.name}>
            <dt><span className="db-row-value-name">{column.name}</span><span className="db-row-value-type">{column.type}</span>
              {(column.columnKey === 'PRI' || column.generated || column.binary) && <span className="db-row-value-tags">{column.columnKey === 'PRI' && <span>Primary key</span>}{column.generated && <span>Generated</span>}{column.binary && <span>Hex</span>}</span>}
            </dt>
            <dd><div className="db-row-value-content">{value === null ? <span className="db-row-missing">NULL</span> : value === '' ? <span className="db-row-missing">empty</span> : <pre>{value}</pre>}</div>
              <button type="button" className="icon-button" aria-label={`Copy ${column.name}`} title={value === null ? 'Copy SQL NULL' : value === '' ? 'Copy empty text' : 'Copy value'} onClick={() => void copy(`field:${column.name}`, value ?? 'NULL', column.name)}>{copied === `field:${column.name}` ? <Check size={15} /> : <Copy size={15} />}</button>
            </dd>
          </div>;
        })}</dl>
      </div>
      <footer className="modal-form-actions db-row-viewer-footer">
        <div className={`db-row-copy-feedback${feedback?.error ? ' error' : ''}`}>
          <span role="status" aria-live="polite">{feedback && !feedback.error ? feedback.text : ''}</span>
          <span role="alert">{feedback?.error ? feedback.text : ''}</span>
        </div>
        <div className="db-row-viewer-actions"><button type="button" className="secondary db-row-copy-all" title="Copy all fields as JSON, preserving NULL, empty text and exact values" onClick={() => void copy('row', JSON.stringify(Object.fromEntries(columns.map(column => [column.name, row[column.name] ?? null])), null, 2), 'Row JSON')}>{copied === 'row' ? <Check size={14} /> : <Copy size={14} />}Copy row JSON</button>
          <button type="button" className="secondary" onClick={onClose}>Close</button>{onEdit && <button type="button" className="primary" onClick={onEdit}><Pencil size={14} />Edit</button>}
        </div>
      </footer>
    </div>
  </div></div>;
}
