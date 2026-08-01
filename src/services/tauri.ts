import { invoke } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import type { MediaItem, ListFilesOptions } from '@/types/media';
import type { Person, UnnamedFace } from '@/types/person';
import type { PairingCodes, DiscoveredHost } from '@/types/sync';
import type { SearchFacetsData } from '@/types/search';

export class TauriError extends Error {
  constructor(
    public command: string,
    public originalError: unknown,
  ) {
    super(`Tauri command '${command}' failed: ${String(originalError)}`);
    this.name = 'TauriError';
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args ?? {});
  } catch (error) {
    throw new TauriError(command, error);
  }
}

function parseJsonArray<T>(raw: string): T[] {
  return JSON.parse(raw) as T[];
}

function parseJsonObject<T>(raw: string): T {
  return JSON.parse(raw) as T;
}

export async function getOs(): Promise<string> {
  return call<string>('get_os');
}

export async function isInitialized(): Promise<boolean> {
  return call<boolean>('is_initialized');
}

export async function markOnboardingComplete(): Promise<void> {
  await call<unknown>('mark_onboarding_complete');
}

export async function scanFiles(): Promise<void> {
  await call<unknown>('scan_files');
}

export async function listFiles(options: ListFilesOptions): Promise<MediaItem[]> {
  const raw = await call<string>('list_files', {
    offset: options.offset,
    limit: options.limit,
    query: options.query ?? '',
    scan: options.scan ?? false,
    favoritesOnly: options.favoritesOnly ?? false,
    videosOnly: options.videosOnly ?? false,
    personId: options.personId ?? null,
    location: options.location ?? null,
    tag: options.tag ?? null,
    dateFrom: options.dateFrom ?? null,
    dateTo: options.dateTo ?? null,
  });
  return parseJsonArray<MediaItem>(raw);
}

export async function getPhotoById(id: number): Promise<MediaItem | null> {
  const raw = await call<string>('get_photo_by_id', { id });
  if (raw === 'null') return null;
  return parseJsonObject<MediaItem>(raw);
}

export async function getPhotoEncodedBatch(ids: number[]): Promise<Record<number, string>> {
  return call<Record<number, string>>('get_photo_encoded_batch', { ids });
}

export async function getPhotosForMapClick(ids: number[]): Promise<MediaItem[]> {
  const raw = await call<string>('get_photos_for_map_click', { ids });
  return parseJsonArray<MediaItem>(raw);
}

export async function toggleFavorite(id: number): Promise<boolean> {
  return call<boolean>('toggle_favorite', { id });
}

export async function setWallpaper(path: string): Promise<void> {
  await call<unknown>('set_wallpaper', { path });
}

export async function getHeatmapData(): Promise<
  Array<{ id: number; latitude: number; longitude: number }>
> {
  const raw = await call<string>('get_heatmap_data');
  return parseJsonArray<{ id: number; latitude: number; longitude: number }>(raw);
}

export async function getMediaServerPort(): Promise<number> {
  return call<number>('get_media_server_port');
}

export async function getIndexingStatus(): Promise<number> {
  return call<number>('get_indexing_status');
}

export async function getUnindexedCount(): Promise<number> {
  return call<number>('get_unindexed_count');
}

export async function indexFaces(): Promise<void> {
  await call<unknown>('index_faces');
}

export async function analyzePhoto(id: number): Promise<void> {
  await call<unknown>('analyze_photo', { id });
}

export async function analyzePhotoModel(id: number, modelId: string): Promise<void> {
  await call<unknown>('analyze_photo_model', { id, modelId });
}

export async function analyzeModel(modelId: string): Promise<void> {
  await call<unknown>('analyze_model', { modelId });
}

export async function abortIndexing(): Promise<void> {
  await call<unknown>('abort_indexing');
}

export async function addDirectory(path: string): Promise<void> {
  await call<unknown>('add_directory', { path });
}

export async function removeDirectory(path: string): Promise<void> {
  await call<unknown>('remove_directory', { path });
}

export async function removeDirectoryFull(path: string): Promise<void> {
  await call<unknown>('remove_directory_full', { path });
}

export async function listDirectories(): Promise<string[]> {
  const raw = await call<string>('list_directories');
  return parseJsonArray<string>(raw);
}

export async function getPeople(): Promise<Person[]> {
  const raw = await call<string>('get_people');
  return parseJsonArray<Person>(raw);
}

export async function getUnnamedFaces(): Promise<Person[]> {
  const raw = await call<string>('get_unnamed_faces');
  return parseJsonArray<Person>(raw);
}

export async function getPersonPhotos(personId: string): Promise<MediaItem[]> {
  const raw = await call<string>('get_person_photos', { personId });
  return parseJsonArray<MediaItem>(raw);
}

export async function getPersonFaces(personId: number): Promise<UnnamedFace[]> {
  const raw = await call<string>('get_person_faces', { personId });
  return parseJsonArray<UnnamedFace>(raw);
}

export async function getFacesForPhoto(photoId: number): Promise<UnnamedFace[]> {
  const raw = await call<string>('get_faces_for_photo', { photoId });
  return parseJsonArray<UnnamedFace>(raw);
}

export async function assignNameToFace(faceId: number, name: string): Promise<void> {
  await call<unknown>('assign_name_to_face', { faceId, name });
}

export async function renamePerson(id: number, newName: string): Promise<void> {
  await call<unknown>('rename_person', { id, newName });
}

export async function mergePeople(fromId: number, toId: number): Promise<void> {
  await call<unknown>('merge_people', { fromId, toId });
}

export async function deleteFace(faceId: number): Promise<void> {
  await call<unknown>('delete_face', { faceId });
}

export async function searchFacets(): Promise<SearchFacetsData> {
  const raw = await call<string>('search_facets');
  return parseJsonObject<SearchFacetsData>(raw);
}

export async function checkModels(): Promise<string[]> {
  return call<string[]>('check_models');
}

export async function downloadModels(models: string[]): Promise<void> {
  await call<unknown>('download_models', { models });
}

export async function saveConfig(key: string, value: string): Promise<void> {
  await call<unknown>('save_config', { key, value });
}

export async function getConfig(): Promise<Record<string, string>> {
  const raw = await call<string>('get_config');
  return parseJsonObject<Record<string, string>>(raw);
}

export async function getLastScanTime(): Promise<string> {
  return call<string>('get_last_scan_time');
}

export async function getLogs(
  limit: number,
): Promise<Array<{ timestamp: string; message: string; level: string }>> {
  const raw = await call<string>('get_logs', { limit });
  return parseJsonArray<{ timestamp: string; message: string; level: string }>(raw);
}

export async function clearLogs(): Promise<void> {
  await call<unknown>('clear_logs');
}

export async function cleanupDatabase(confirm: boolean): Promise<void> {
  await call<unknown>('cleanup_database', { confirm });
}

export async function resolvePhotoLocations(): Promise<void> {
  await call<unknown>('resolve_photo_locations');
}

export async function getLocationNames(): Promise<void> {
  await call<unknown>('get_location_names');
}

export async function initializeSyncFolder(path: string): Promise<void> {
  await call<unknown>('initialize_sync_folder', { path });
}

export async function requestStartSync(): Promise<void> {
  await call<unknown>('request_start_sync');
}

export async function autoReconnect(): Promise<boolean> {
  return call<boolean>('auto_reconnect');
}

export async function listDevices(): Promise<
  Array<{
    id: string;
    title: string;
    icon: string;
    os: string;
    photo_count: number;
    video_count: number;
    host: string;
    subtitle: string;
  }>
> {
  const raw = await call<string>('list_devices');
  return parseJsonArray(raw);
}

export async function removeDevice(id: string): Promise<void> {
  await call<unknown>('remove_device', { id });
}

export async function renameDevice(id: string, newName: string): Promise<void> {
  await call<unknown>('rename_device', { id, newName });
}

export async function generatePairingCodes(): Promise<PairingCodes> {
  return call<PairingCodes>('generate_pairing_codes');
}

export async function hashPairingCode(input: string): Promise<string> {
  return call<string>('hash_pairing_code', { input });
}

export async function startWebrtcSession(
  roomId: string,
  isInitiator: boolean,
  signalingUrl: string,
): Promise<void> {
  await call<unknown>('start_webrtc_session', { roomId, isInitiator, signalingUrl });
}

export async function stopWebrtcSession(): Promise<void> {
  await call<unknown>('stop_webrtc_session');
}

export async function startLanHost(
  roomId: string,
  isInitiator: boolean,
): Promise<{ ip: string; port: number }> {
  return call<{ ip: string; port: number }>('start_lan_host', { roomId, isInitiator });
}

export const isAndroid = platform() === 'android';

export async function pingMdnsPlugin(): Promise<boolean> {
  if (!isAndroid) return false;
  const r = await invoke<{ ok: boolean }>('plugin:mdns|ping');
  return r.ok;
}

export async function discoverLanDevices(timeoutSecs: number = 3): Promise<DiscoveredHost[]> {
  if (isAndroid) {
    const result = await invoke<{ hosts: string }>('plugin:mdns|discover', { timeoutSecs });
    return JSON.parse(result.hosts);
  }
  return call<DiscoveredHost[]>('discover_lan_devices', { timeoutSecs });
}

export async function joinNetwork(roomId: string): Promise<void> {
  await call<unknown>('join_network', { roomId });
}
