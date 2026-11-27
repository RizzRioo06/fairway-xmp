/** UI labels only. Native code selects and enforces the credential backend. */
export function credentialStoreName(platform: string): string {
  switch (platform.toLowerCase()) {
    case 'macos': case 'darwin': return 'macOS Keychain';
    case 'windows': case 'win32': return 'Windows Credential Manager';
    case 'linux': return 'Linux Secret Service';
    default: return 'system secure storage';
  }
}
