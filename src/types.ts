export type ServiceId = 'nginx' | 'php' | 'mysql' | 'phpmyadmin' | 'mail';
export type Ports = { website: number; phpmyadmin: number; mysql: number; php: number; mail: number; smtp: number };
export type ServiceStatus = {
  id: ServiceId; running: boolean; ready: boolean; port: number;
  pid: number | null; message: string | null;
};
export type ProjectConfig = { name: string; domain: string; root: string; https: boolean; indexFallback: boolean };
export type Project = ProjectConfig & { available: boolean; domainReady: boolean };
export type Snapshot = {
  ports: Ports; services: ServiceStatus[]; documentRoot: string; projects: Project[]; sitesError: string | null; domainsPending: boolean; httpsTrusted: boolean; os: string; dataDir: string;
  runtimeReady: boolean; runtimeVersions: Record<string, string> | null; mailAvailable: boolean;
  phpVersion: string; phpVersions: Record<string, string>;
  pathInstalled: boolean; cliPhp: string | null; cliComposer: string | null;
  pathPhp: string | null; pathComposer: string | null; version: string;
};
export type UpdateInfo = { configured: boolean; version: string | null; notes: string | null };
export type UpdateProgress = { phase: 'downloading' | 'verifying' | 'preparing' | 'installing'; downloaded?: number; total?: number | null };
export type MailAddress = { Name: string; Address: string };
export type MailSummary = { ID: string; Read: boolean; From: MailAddress | null; To: MailAddress[] | null; Subject: string; Created: string; Snippet: string; Attachments: number };
export type MailList = { messages: MailSummary[]; messages_count: number; messages_unread: number };
export type MailAttachment = { PartID: string; FileName: string; ContentType: string; Size: number };
export type MailMessage = { ID: string; From: MailAddress | null; To: MailAddress[] | null; Cc: MailAddress[] | null; Subject: string; Date: string; HTML: string; Text: string; Attachments: MailAttachment[] | null };
