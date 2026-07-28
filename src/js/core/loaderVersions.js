import { invoke } from './tauri.js';

let neoforgeVersionsCache = null;
let forgeVersionsCache = null;

export function resetLoaderVersionsCache() {
    neoforgeVersionsCache = null;
    forgeVersionsCache = null;
}

function mcVersionToNeoforgePrefix(mcVersion) {
    const parts = mcVersion.split('.');
    if (parts[0] === '1') {
        if (parts.length >= 3) return `${parts[1]}.${parts[2]}`;
        return `${parts[1]}.0`;
    }
    if (parts.length >= 3) return `${parts[0]}.${parts[1]}.${parts[2]}`;
    return `${parts[0]}.${parts[1]}.0`;
}

function compareVersionParts(a, b) {
    const pa = a.split(/[.-]/).map(p => (isNaN(parseInt(p, 10)) ? p : parseInt(p, 10)));
    const pb = b.split(/[.-]/).map(p => (isNaN(parseInt(p, 10)) ? p : parseInt(p, 10)));
    const len = Math.max(pa.length, pb.length);
    for (let i = 0; i < len; i++) {
        const x = pa[i], y = pb[i];
        if (x === undefined) return -1;
        if (y === undefined) return 1;
        if (typeof x === 'number' && typeof y === 'number') {
            if (x !== y) return x - y;
        } else {
            const sx = String(x), sy = String(y);
            if (sx !== sy) return sx < sy ? -1 : 1;
        }
    }
    return 0;
}

async function resolveLatestNeoforge(mcVersion) {
    if (!neoforgeVersionsCache) {
        neoforgeVersionsCache = await invoke('fetch_neoforge_versions');
    }
    const prefix = mcVersionToNeoforgePrefix(mcVersion);
    const matches = neoforgeVersionsCache
        .filter(v => v.startsWith(`${prefix}.`))
        .sort(compareVersionParts);
    if (matches.length === 0) return null;

    const build = matches[matches.length - 1];
    console.log(`[loaderVersions] NeoForge más reciente para ${mcVersion}: ${build}`);
    return {
        versionId: `neoforge-${build}`,
        loaderUrl: `https://maven.neoforged.net/releases/net/neoforged/neoforge/${build}/neoforge-${build}-installer.jar`
    };
}

async function resolveLatestForge(mcVersion) {
    if (!forgeVersionsCache) {
        forgeVersionsCache = await invoke('fetch_forge_versions');
    }
    const versions = forgeVersionsCache[mcVersion] || [];
    if (versions.length === 0) return null;

    const sorted = versions
        .map(full => ({
            full,
            forgeOnly: full.startsWith(`${mcVersion}-`) ? full.slice(mcVersion.length + 1) : full
        }))
        .sort((a, b) => compareVersionParts(a.forgeOnly, b.forgeOnly));

    const { full } = sorted[sorted.length - 1];
    const forgeOnly = full.startsWith(`${mcVersion}-`) ? full.slice(mcVersion.length + 1) : full;
    console.log(`[loaderVersions] Forge más reciente para ${mcVersion}: ${forgeOnly}`);
    return {
        versionId: `${mcVersion}-forge-${forgeOnly}`,
        loaderUrl: `https://maven.minecraftforge.net/net/minecraftforge/forge/${full}/forge-${full}-installer.jar`
    };
}

export async function resolveLatestLoaderVersion(loaderName, mcVersion) {
    const name = (loaderName || '').toLowerCase();
    if (name === 'neoforge') return resolveLatestNeoforge(mcVersion);
    if (name === 'forge') return resolveLatestForge(mcVersion);
    return null;
}

export async function resolveRemoteEntry(remote) {
    if (remote.version_id && remote.loader_url) return remote;

    const resolved = await resolveLatestLoaderVersion(remote.loader_name, remote.mc_version);
    if (!resolved) {
        throw new Error(`No se encontró ninguna build de ${remote.loader_name} para Minecraft ${remote.mc_version}`);
    }
    return { ...remote, version_id: resolved.versionId, loader_url: resolved.loaderUrl };
}