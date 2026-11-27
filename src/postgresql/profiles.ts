// Native metadata storage; optional secrets are written only to the OS vault.
import { invoke } from '../bridge';
import type { PgSession } from './explorer-types';
export type Profile = { id: string; revision: string; name: string; host: string; port: number; user: string; database: string; rememberPassword: boolean };
export type SaveProfile = { confirmed: true; profile: Profile; password?: string };
export function listProfiles(): Promise<Profile[]> { return invoke('postgres_profiles_list'); }
export function saveProfile(request: SaveProfile): Promise<Profile> { return invoke('postgres_profile_save', { request }); }
export function removeProfile(profile: Profile): Promise<void> { return invoke('postgres_profile_remove', { request: { confirmed: true, id: profile.id, revision: profile.revision } }); }
export function openProfile(profile: Profile, password?: string): Promise<PgSession> { return invoke('postgres_profile_open', { request: { confirmed: true, id: profile.id, revision: profile.revision, password } }); }
