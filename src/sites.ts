import type { ProjectConfig, Project, Snapshot } from './types';

export function siteUrl(site: ProjectConfig, websitePort: number) {
  return site.https ? `https://${site.domain}.test:8443/` : `http://${site.domain}.test:${websitePort}/`;
}

export function siteStatus(site: Project, data: Snapshot) {
  if (!site.available) return 'Folder missing';
  if (!site.domainReady) return data.domainsApprovalPending ? 'Waiting for macOS approval' : 'Address setup needed';
  if (!data.runtimeReady) return 'Web runtime missing';
  if (site.https && !data.httpsTrusted) return 'HTTPS setup needed';
  const server = data.services.find(service => service.id === 'nginx');
  if (!server?.ready) return server?.running ? 'Web server not ready' : 'Web server stopped';
  return 'Ready to open';
}

export function siteSaveMessage(site: Project, data: Snapshot) {
  const status = siteStatus(site, data);
  return status === 'Ready to open'
    ? `${site.name} is ready at ${siteUrl(site, data.ports.website)}`
    : `${site.name} saved. ${status}.`;
}

export function hasPublicRoot(root: string) {
  return root.replace(/[\\/]+$/, '').split(/[\\/]/).pop()?.toLowerCase() === 'public';
}

export function insideDocumentRoot(root: string, documentRoot: string, os: string) {
  const normalize = (path: string) => {
    const value = path.replace(/\\/g, '/').replace(/\/+$/, '');
    return os === 'windows' ? value.toLowerCase() : value;
  };
  const path = normalize(root), parent = normalize(documentRoot);
  return !!path && !!parent && (path === parent || path.startsWith(`${parent}/`));
}
