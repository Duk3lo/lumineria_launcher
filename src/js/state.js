const { invoke } = window.__TAURI__.core;

export let PROFILES = {};
export let selectedProfileId = null;
export let AUTH_SESSION = null; // { username, uuid, accessToken, userType }
export let SETTINGS = { ramMinMb: 1024, ramMaxMb: 4096, javaArgsExtra: "" };

let baseDirectoryCache = null;

/**
 * Carga las instancias locales desde ui/profiles.json.
 * Por ahora es un archivo estático junto al resto del frontend
 * (mismo lugar que index.html), pensado para probar el launcher
 * con instancias reales antes de conectarlo a un backend remoto.
 */
export async function fetchProfiles() {
    const res = await fetch('./profiles.json');
    if (!res.ok) {
        throw new Error(`No se pudo cargar profiles.json (${res.status})`);
    }
    PROFILES = await res.json();
    return PROFILES;
}

export function setProfileSelection(id) {
    selectedProfileId = id;
}

export async function getBaseDirectory() {
    if (!baseDirectoryCache) {
        baseDirectoryCache = await invoke('get_default_path');
    }
    return baseDirectoryCache;
}

export async function getInstanceDir(profileId) {
    const baseDir = await getBaseDirectory();
    return `${baseDir}/instances/${profileId}`;
}

// ---- Settings (RAM, args extra de Java) ----

export async function loadSettings() {
    const baseDir = await getBaseDirectory();
    SETTINGS = await invoke('load_settings', { baseDir });
    return SETTINGS;
}

export async function saveSettings(partialSettings) {
    const baseDir = await getBaseDirectory();
    SETTINGS = { ...SETTINGS, ...partialSettings };
    await invoke('save_settings', { baseDir, settings: SETTINGS });
    return SETTINGS;
}

export async function getSystemRamMb() {
    return await invoke('get_system_ram_mb');
}

// ---- Estado de instalación / mods (estilo CurseForge) ----

export async function getInstanceStatus(profileId) {
    const instanceDir = await getInstanceDir(profileId);
    return await invoke('get_instance_status', { instanceDir });
}

export async function listMods(profileId) {
    const instanceDir = await getInstanceDir(profileId);
    return await invoke('list_mods', { instanceDir });
}

export async function toggleMod(profileId, filename, enable) {
    const instanceDir = await getInstanceDir(profileId);
    return await invoke('toggle_mod', { instanceDir, filename, enable });
}

// ---- Login ----

export function setAuthSession(session) {
    AUTH_SESSION = session;
}

export async function loginOffline(username) {
    AUTH_SESSION = await invoke('offline_login', { username });
    return AUTH_SESSION;
}

export async function loginMicrosoftStart() {
    // -> { deviceCode, userCode, verificationUri, interval, expiresIn }
    return await invoke('ms_login_start');
}

export async function loginMicrosoftPoll(deviceCode, interval, expiresIn) {
    AUTH_SESSION = await invoke('ms_login_poll', { deviceCode, interval, expiresIn });
    return AUTH_SESSION;
}

// ---- Sesión persistida (para no tener que loguearse cada vez que se abre) ----

export async function saveSession() {
    if (!AUTH_SESSION) return;
    const baseDir = await getBaseDirectory();
    await invoke('save_session', { baseDir, session: AUTH_SESSION });
}

export async function loadSession() {
    const baseDir = await getBaseDirectory();
    const session = await invoke('load_session', { baseDir });
    if (session) {
        AUTH_SESSION = session;
    }
    return AUTH_SESSION;
}

export async function clearSession() {
    const baseDir = await getBaseDirectory();
    await invoke('clear_session', { baseDir });
    AUTH_SESSION = null;
}