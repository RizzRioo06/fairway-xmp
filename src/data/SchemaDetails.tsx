import { Plus } from 'lucide-react';
import type { ForeignKey, Structure, TableIndex } from './types';

export default function SchemaDetails({ structure, database, sqlite, writable, blocked, canCreate, onCreate, onRemove }: {
  structure: Structure; database: string; sqlite: boolean; writable: boolean; blocked: boolean;
  canCreate: boolean; onCreate: () => void; onRemove: (index: TableIndex) => void;
}) {
  const indexes = structure.indexes ?? [];
  const outgoing = structure.foreignKeys ?? [];
  const incoming = structure.referencedBy ?? [];
  function qualified(db: string | null, table: string) { return db && db !== database ? `${db}.${table}` : table; }
  function relationship(fk: ForeignKey, i: number) {
    return <li key={`${fk.database ?? ''}/${fk.table}/${fk.name}/${i}`} className="db-foreign-key">
      <strong>{fk.name}</strong>
      {fk.columns.map((column, n) => <code key={n}>{qualified(fk.database, fk.table)}.{column} → {qualified(fk.referencedDatabase, fk.referencedTable)}.{fk.referencedColumns[n] ?? '(primary key)'}</code>)}
      <span>On update: {fk.onUpdate} · On delete: {fk.onDelete}</span>
    </li>;
  }
  return <>
    <section className="db-schema-section" aria-label="Indexes">
      <div className="db-row-tools"><h3>Indexes <span>{indexes.length}</span></h3><button className="secondary" disabled={blocked || !writable || !canCreate} onClick={onCreate}><Plus size={14} />Index</button></div>
      {indexes.length ? <ul className="db-index-list">{indexes.map(index => <li key={index.name} className="db-index-item">
        <div className="db-index-heading"><strong>{index.name}</strong><span className="db-index-kind">{index.primary ? 'Primary key' : index.unique ? 'Unique' : 'Index'}</span>{index.removable && <button className="db-danger-text" disabled={blocked || !writable} aria-label={`Remove index ${index.name}`} onClick={() => onRemove(index)}>Remove</button>}</div>
        <p className="db-index-parts">{index.parts.map((part, i) => <code key={i}>{part.name ?? part.expression ?? '(expression)'}{part.prefix !== null ? `(${part.prefix})` : ''}{part.direction === 'DESC' ? ' DESC' : ''}{part.collation && part.collation !== 'BINARY' ? ` COLLATE ${part.collation}` : ''}</code>)}</p>
        <p className="db-hint">{[index.type, index.details, index.reason].filter(Boolean).join(' · ')}</p>
        {index.sql && <details className="db-ddl"><summary>Index SQL</summary><pre>{index.sql}</pre></details>}
      </li>)}</ul> : <p className="db-hint">No indexes on this {structure.kind === 'VIEW' ? 'view' : 'table'}.</p>}
    </section>
    <section className="db-schema-section" aria-label="Foreign keys">
      <h3>Foreign keys</h3><p className="db-hint">Relationships are shown for inspection. Create or change foreign keys in SQL.</p>
      {sqlite && <p className="db-hint">Foreign keys are enforced by this browser. Your application configures its own connection.</p>}
      <h4>References <span>{outgoing.length}</span></h4>{outgoing.length ? <ul className="db-foreign-keys">{outgoing.map(relationship)}</ul> : <p className="db-hint">No foreign keys defined on this table.</p>}
      <h4>Referenced by <span>{incoming.length}</span></h4>{incoming.length ? <ul className="db-foreign-keys">{incoming.map(relationship)}</ul> : <p className="db-hint">No tables reference this table through a foreign key.</p>}
    </section>
  </>;
}
