const { invoke } = window.__TAURI__.core;

export let PROFILES = {};
export let selectedProfileId = null;
export let AUTH_SESSION = null;
export let SETTINGS = { ramMinMb: 1024, ramMaxMb: 4096, javaArgsExtra: "" };

let baseDirectoryCache = null;

// 1. CARGA DE PERFILES DESDE RUST
export async function fetchProfiles() {
    const baseDir = await getBaseDirectory();
    PROFILES = await invoke('load_profiles', { baseDir });
    return PROFILES;
}

// 2. GUARDAR NUEVOS PERFILES (PERSISTENCIA)
export async function saveProfileToDisk(profileId, profileData) {
    const baseDir = await getBaseDirectory();
    PROFILES[profileId] = profileData;
    await invoke('save_profile', { baseDir, profileId, profileData });
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

// 3. LOGICA VANILLA vs CUSTOM PATH
export async function getInstanceDir(profileId) {
    const profile = PROFILES[profileId];
    if (!profile) throw new Error("Perfil no encontrado");

    // Si es Vanilla, usa la carpeta .minecraft por defecto del sistema
    if (profile.loader_name.toLowerCase() === 'vanilla') {
        return await invoke('get_minecraft_default_path');
    }
    
    // Si es modificado (Fabric, Forge), usa tu carpeta aislada en LumineriaData
    const baseDir = await getBaseDirectory();
    return `${baseDir}/instances/${profileId}`;
}

export async function resetInstanceLibraries(profileId) {
    const instanceDir = await getInstanceDir(profileId);
    await invoke('reset_instance_libraries', { instanceDir });
}

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

export function setAuthSession(session) {
    AUTH_SESSION = session;
}

export async function loginOffline(username) {
    AUTH_SESSION = await invoke('offline_login', { username });
    return AUTH_SESSION;
}

export async function loginMicrosoftStart() {
    return await invoke('ms_login_start');
}

export async function loginMicrosoftPoll(deviceCode, interval, expiresIn) {
    AUTH_SESSION = await invoke('ms_login_poll', { deviceCode, interval, expiresIn });
    return AUTH_SESSION;
}

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