import { useId, useState } from 'react';
import Dialog from '../data/Dialog';
import type { PgOutput } from './explorer-types';
import { cellPreview, resultOutcome } from './result-presentation';
export default function PgResult({ output, title, rowOffset = 0 }: { output: PgOutput; title: string; rowOffset?: number }) {
  const [cell, setCell] = useState<{ label: string; value: string | null } | null>(null);
  const [wrap, setWrap] = useState(false);
  const helpId = useId();
  return <section className="pg-result" aria-label={title}>
    <div className="pg-result-toolbar"><div className="pg-result-summary"><strong>{output.rows.length} {output.rows.length === 1 ? 'row' : 'rows'} returned</strong><span>{output.columns.length} {output.columns.length === 1 ? 'column' : 'columns'}</span><span className={`pg-outcome pg-outcome-${output.outcome}`}>{resultOutcome(output)}</span></div>{output.columns.length > 0 && <label className="pg-wrap-toggle"><input type="checkbox" checked={wrap} onChange={e => setWrap(e.target.checked)} />Wrap cell text</label>}</div>
    {output.affectedRows !== null && <p className="db-hint pg-command"><code>{output.command || 'Result'}</code> · {output.affectedRows} {output.affectedRows === '1' ? 'row' : 'rows'} reported by PostgreSQL</p>}
    {output.truncated && <p className="pg-warning" role="status">Result limit reached. This is a partial result; more rows may exist. The statement was cancelled at its result limit.</p>}
    {output.outcome === 'unknown' && <p className="pg-warning" role="alert">The operation's outcome is unknown. Check the database before running the statement again.</p>}
    {output.warnings.map((warning, index) => <p key={index} className="db-hint">{warning}</p>)}
    {output.columns.length > 0 ? <>
      <p className="db-hint pg-table-help" id={helpId}>Click a value to see its full text. <span className="pg-null">NULL</span> means no value; <span className="pg-empty-value">Empty string</span> is a stored blank.</p>
      <div className={`db-grid-scroll pg-grid-scroll${wrap ? ' pg-wrap-cells' : ''}`} role="region" aria-label={`${title} table`} aria-describedby={helpId} tabIndex={0}><table className="db-grid pg-data-grid"><thead><tr><th className="pg-row-number" scope="col" aria-label="Row number">#</th>{output.columns.map((column, index) => <th key={index} scope="col"><span>{column.name}</span><small title={`OID ${column.typeOid}; modifier ${column.typeModifier}`}>{column.typeName}</small></th>)}</tr></thead><tbody>{output.rows.map((row, ri) => <tr key={ri}><th className="pg-row-number" scope="row">{rowOffset + ri + 1}</th>{output.columns.map((column, ci) => <td key={ci}><button className="pg-cell" aria-label={`View ${column.name}, column ${ci + 1}, row ${rowOffset + ri + 1}`} onClick={() => setCell({ label: `${column.name} · row ${rowOffset + ri + 1}`, value: row[ci] })}>{row[ci] === null ? <span className="pg-null">NULL</span> : row[ci] === '' ? <span className="pg-empty-value">Empty string</span> : cellPreview(row[ci])}</button></td>)}</tr>)}</tbody></table>{!output.rows.length && <p className="db-grid-empty">No rows match this view. Try another filter or refresh the table.</p>}</div>
    </> : <p className="db-hint">No table result returned. Check the outcome above before running the statement again.</p>}
    {cell && <Dialog title="PostgreSQL value" busy={false} onClose={() => setCell(null)} onSubmit={() => {}} showSubmit={false} cancel="Close"><p>{cell.label}</p><p className="db-hint">Full returned text. NULL and empty strings remain distinct.</p>{cell.value === null ? <p className="pg-null">NULL — no value</p> : cell.value === '' ? <p className="pg-empty-value">Empty string — stored blank</p> : <textarea aria-label="Exact PostgreSQL value" readOnly value={cell.value} rows={10} spellCheck={false} />}</Dialog>}
  </section>;
}
