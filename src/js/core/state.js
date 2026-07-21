import { invoke } from './tauri.js';

export let PROFILES = {};
export let selectedProfileId = null;
export let AUTH_SESSION = null;
export let SETTINGS = { ramMinMb: 1024, ramMaxMb: 4096, javaArgsExtra: "" };

let baseDirectoryCache = null;
export async function fetchProfiles() {
    const baseDir = await getBaseDirectory();
    PROFILES = await invoke('load_profiles', { baseDir });
    return PROFILES;
}
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
export async function getInstanceDir(profileId) {
    const profile = PROFILES[profileId];
    if (!profile || profile.loader_name.toLowerCase() === 'vanilla') {
        return await invoke('get_minecraft_default_path');
    }
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

export async function deleteProfileFromDisk(profileId) {
    const baseDir = await getBaseDirectory();
    await invoke('delete_profile', { baseDir, profileId });
    delete PROFILES[profileId];
}

export async function syncInstalledProfilesFromDatabase() {
    let database;
    try {
        const baseDir = await getBaseDirectory();
        database = await invoke('fetch_official_modpacks', { baseDir });
    } catch (e) {
        console.warn('No se pudo comprobar actualizaciones del catálogo:', e);
        return;
    }

    const FIELDS_TO_SYNC = ['title', 'mc_version', 'version_id', 'java_version', 'loader_name', 'loader_url', 'packwiz_url', 'image'];

    for (const id of Object.keys(PROFILES)) {
        const local = PROFILES[id];
        if (!local.is_official) continue;

        const remote = database[id];
        if (!remote) continue;

        let changed = false;
        const merged = { ...local };

        const versionChanged = remote.version_id !== undefined && remote.version_id !== local.version_id;
        const loaderChanged = remote.loader_name !== undefined && remote.loader_name !== local.loader_name;
        const javaChanged = remote.java_version !== undefined && remote.java_version !== local.java_version;

        for (const field of FIELDS_TO_SYNC) {
            if (remote[field] !== undefined && remote[field] !== local[field]) {
                merged[field] = remote[field];
                changed = true;
            }
        }
        if (!changed) continue;
        if (versionChanged || loaderChanged || javaChanged) {
            try {
                const instanceDir = await getInstanceDir(id);
                await invoke('cleanup_old_version', { instanceDir, oldVersionId: local.version_id });
            } catch (e) {
                console.warn(`No se pudo limpiar la versión anterior de ${merged.title}:`, e);
            }
        }

        await saveProfileToDisk(id, merged);
        console.log(`"${merged.title}" actualizado automáticamente desde el catálogo.`);
    }
}


export async function syncSingleProfileFromDatabase(id) {
    let database;
    try {
        const baseDir = await getBaseDirectory();
        database = await invoke('fetch_official_modpacks', { baseDir });
    } catch (e) {
        throw new Error("No se pudo conectar al servidor para actualizar.");
    }

    const remote = database[id];
    if (!remote) throw new Error("Esta instancia no existe en la base de datos oficial.");

    const local = PROFILES[id];
    let changed = false;
    const merged = { ...local, is_official: true };

    const FIELDS_TO_SYNC = ['title', 'mc_version', 'version_id', 'java_version', 'loader_name', 'loader_url', 'packwiz_url', 'image'];

    const versionChanged = remote.version_id !== local.version_id;
    const loaderChanged = remote.loader_name !== local.loader_name;
    const javaChanged = remote.java_version !== local.java_version;

    for (const field of FIELDS_TO_SYNC) {
        if (remote[field] !== undefined && remote[field] !== local[field]) {
            merged[field] = remote[field];
            changed = true;
        }
    }

    if (versionChanged || loaderChanged || javaChanged) {
        try {
            const instanceDir = await getInstanceDir(id);
            await invoke('cleanup_old_version', { instanceDir, oldVersionId: local.version_id });
        } catch (e) { }
    }

    await saveProfileToDisk(id, merged);
    return changed;
}
