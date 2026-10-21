export type Row = Record<string, string | null>;
export type Column = {
  name: string; type: string; nullable: boolean; defaultValue: string | null;
  extra: string; columnKey: string; expression: string; binary: boolean; generated: boolean;
};
export type Table = { name: string; kind: string };
export type TableIndex = {
  name: string; unique: boolean; primary: boolean; type: string;
  parts: { name: string | null; expression: string | null; direction: string; prefix: string | null; collation: string | null }[];
  details: string; sql: string | null; removable: boolean; reason: string;
};
export type ForeignKey = {
  name: string; database: string | null; table: string; columns: string[];
  referencedDatabase: string | null; referencedTable: string; referencedColumns: (string | null)[];
  onUpdate: string; onDelete: string;
};
export type Structure = { columns: Column[]; key: string[]; kind: string; sql?: string; indexes?: TableIndex[]; foreignKeys?: ForeignKey[]; referencedBy?: ForeignKey[] };
export type RowRef = { key: Row; version: string };
export type LargeFields = Record<string, { length: number }>;
export type Page = Structure & { rows: Row[]; rowRefs?: RowRef[]; truncated?: LargeFields[]; total: string; offset: number; limit: number };
export type QueryResult = { sets: { columns: string[]; rows: (string | null)[][]; affected: number; truncated: boolean; summary?: string }[]; elapsedMs: number; resultsTruncated?: boolean };
export type Definition = {
  name: string; type: string; length: string; scale: string; unsigned: boolean;
  nullable: boolean; primary: boolean; autoIncrement: boolean;
  defaultMode: 'none' | 'null' | 'value' | 'current_timestamp'; defaultValue: string;
};
export const columnTypes = ['INT', 'BIGINT', 'SMALLINT', 'TINYINT', 'DECIMAL', 'FLOAT', 'DOUBLE', 'CHAR', 'VARCHAR', 'TEXT', 'LONGTEXT', 'BOOLEAN', 'DATE', 'DATETIME', 'TIMESTAMP', 'JSON', 'BLOB'];
export function newColumn(name = '', sqlite = false): Definition {
  return { name, type: 'VARCHAR', length: '255', scale: '2', unsigned: false, nullable: sqlite, primary: false, autoIncrement: false, defaultMode: 'none', defaultValue: '' };
}
export function fromColumn(c: Column): Definition {
  const parts = /^(\w+)(?:\((\d+)(?:,(\d+))?\))?/i.exec(c.type);
  const current = /^current_timestamp(?:\(\d*\))?$/i.test(c.defaultValue ?? '');
  return { ...newColumn(c.name), type: parts?.[1].toUpperCase() ?? c.type, length: parts?.[2] ?? '255', scale: parts?.[3] ?? '2', unsigned: /unsigned/i.test(c.type), nullable: c.nullable, primary: c.columnKey === 'PRI', autoIncrement: /auto_increment/i.test(c.extra), defaultMode: current ? 'current_timestamp' : c.defaultValue === null ? (c.nullable ? 'null' : 'none') : 'value', defaultValue: c.defaultValue ?? '' };
}
// Preserve advanced definitions by directing their edits to SQL instead of silently losing attributes.
export function editableColumn(c: Column): boolean {
  return !c.generated && columnTypes.includes(fromColumn(c).type) && !/on update|invisible|zerofill/i.test(`${c.type} ${c.extra}`) && (!/DEFAULT_GENERATED/i.test(c.extra) || fromColumn(c).defaultMode === 'current_timestamp') && !/\(\d+\)/.test(c.type.replace(/^(?:varchar|char|decimal)\([^)]*\)/i, ''));
}
