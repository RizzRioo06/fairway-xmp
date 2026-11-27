import type { PgOutput } from './explorer-types.ts';
/** Shorten only the display preview; callers keep the exact value for inspection. */
export function cellPreview(value: string, limit = 160): string {
  let preview = ''; let count = 0;
  for (const character of value) {
    if (count === limit) return `${preview}…`;
    preview += character; count++;
  }
  return preview;
}
export function resultOutcome(output: Pick<PgOutput, 'outcome'>): string {
  switch (output.outcome) {
    case 'committed': return 'Changes committed';
    case 'unknown': return 'Outcome unknown — verify before retrying';
    case 'notStarted': return 'Not started';
    case 'notCommitted': return 'Transaction not committed';
  }
}
