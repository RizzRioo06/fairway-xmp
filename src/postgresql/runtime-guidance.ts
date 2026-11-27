/** Recognize only the native missing-runtime failures; don't relabel auth/TLS errors. */
export function needsClientRuntime(error: { code?: string; text: string } | null): boolean {
  return error?.code === 'unavailable' && ['Explicit runtime provisioning is required', 'Runtime not provisioned'].includes(error.text);
}
export const clientRuntimeExplanation = 'Fairway needs its verified PostgreSQL connection tools in this app profile before it can connect to your running server. Preparing them does not initialize a database or start, stop or change your server.';
