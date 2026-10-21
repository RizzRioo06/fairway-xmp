type Key = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'shiftKey' | 'altKey'>;

export function terminalShortcut(event: Key, macos: boolean): 'find' | 'increase' | 'decrease' | 'reset' | null {
  const modifier = macos ? event.metaKey && !event.ctrlKey : event.ctrlKey && event.shiftKey && !event.metaKey;
  if (!modifier || event.altKey) return null;
  if (event.key.toLowerCase() === 'f') return 'find';
  if (event.code === 'Digit0' || event.key === '0') return 'reset';
  if (event.code === 'Minus' || event.key === '-') return 'decrease';
  if (event.code === 'Equal' || event.key === '+' || event.key === '=') return 'increase';
  return null;
}
