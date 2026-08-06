import { PROFILES, getBaseDirectory, getInstanceDir, AUTH_SESSION, SETTINGS, resetInstanceLibraries, saveProfileToDisk, sessionNeedsRefresh, ensureFreshSession } from '../../core/state.js';
import { invoke, listen } from '../../core/tauri.js';
import { updateStatus, updateCardProgress, setCardPlayState, refreshCardStatus, setCardPreparing } from '../../ui/ui.js';
import { setInstanceRunning, setInstancePreparing } from './instanceDetail.js';

const syncingInstances = new Set();

const cancelRequested = new Set();

export function requestCancelPreparation(profileId) {
    cancelRequested.add(profileId);
}

function checkCancelled(profileId) {
    if (cancelRequested.has(profileId)) {
        cancelRequested.delete(profileId);
        const err = new Error('Cancelado por el usuario');
        err.isCancelled = true;
        throw err;
    }
}

export function isSyncing(profileId) {
    return syncingInstances.has(profileId);
}

export async function sincronizarModpack(profileId, { silent = false } = {}) {
    const profile = PROFILES[profileId];
    if (!profile || !profile.packwiz_url) return;
    if (syncingInstances.has(profileId)) return;

    syncingInstances.add(profileId);
    document.dispatchEvent(new CustomEvent('lumineria:sync-state-changed', { detail: { id: profileId, syncing: true } }));
    try {
        const reachable = await invoke('check_url_reachable', { url: profile.packwiz_url });
        if (!reachable) {
            const err = new Error('Sin conexión al servidor de mods.');
            err.isConnectionError = true;
            throw err;
        }

        const baseDir = await getBaseDirectory();
        const instanceDir = await getInstanceDir(profileId);
        const installersDir = `${baseDir}/installers`;

        await invoke('ensure_dir', { path: instanceDir });
        await invoke('ensure_dir', { path: installersDir });

        let javaPath = "java";
        if (profile.java_version) {
            try {
                javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
            } catch {
                await invoke('download_java_command', { version: profile.java_version, baseDir });
                javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
            }
        }

        const packwizUrl = "https://github.com/packwiz/packwiz-installer-bootstrap/releases/latest/download/packwiz-installer-bootstrap.jar";
        const packwizPath = `${installersDir}/packwiz-installer-bootstrap.jar`;

        if (!silent) updateStatus(`Sincronizando mods de ${profile.title}...`);
        await invoke('download_generic_file', { url: packwizUrl, destPath: packwizPath });
        await invoke('execute_jar', {
            javaPath,
            jarPath: packwizPath,
            args: ['-g', profile.packwiz_url],
            workDir: instanceDir
        });

        document.dispatchEvent(new CustomEvent('lumineria:mods-updated', { detail: { id: profileId } }));

        if (!silent) updateStatus(`Mods de ${profile.title} actualizados.`);
    } catch (e) {
        if (!silent) updateStatus(`Error sincronizando mods: ${e}`);
        throw e;
    } finally {
        syncingInstances.delete(profileId);
        document.dispatchEvent(new CustomEvent('lumineria:sync-state-changed', { detail: { id: profileId, syncing: false } }));
    }
}

export function getRecommendedJava(mcVersion) {
    if (!mcVersion) return 17;
    const parts = mcVersion.split('.');
    if (parts[0] !== '1') {
        const year = parseInt(parts[0], 10);
        if (!isNaN(year) && year >= 26) {
            return 25;
        }
        return 21;
    }

    const minor = parseInt(parts[1] || "0");
    const patch = parseInt(parts[2] || "0");
    if (minor <= 16) return 8;
    if (minor === 17) return 16;
    if (minor >= 18 && minor <= 20) {
        if (minor === 20 && patch >= 5) return 21;
        return 17;
    }
    if (minor >= 21) return 21;
    return 17;
}

export async function prepararCliente(profileId, profile, { force = false, isLocal = false } = {}) {
    checkCancelled(profileId);
    if (!profile.java_version) {
        profile.java_version = getRecommendedJava(profile.mc_version);
    }

    const baseDir = await getBaseDirectory();
    const instanceDir = isLocal
        ? await invoke('get_minecraft_default_path')
        : await getInstanceDir(profileId);
    const installersDir = `${baseDir}/installers`;
    const targetVersionId = profile.version_id || profile.mc_version;

    if (force) {
        updateStatus("Limpiando instalación anterior...");
        updateCardProgress(profileId, 2, 'Limpiando archivos previos...');
        await resetInstanceLibraries(profileId);
    }
    checkCancelled(profileId);

    await invoke('ensure_dir', { path: instanceDir });
    await invoke('ensure_dir', { path: installersDir });
    await invoke('ensure_launcher_profile', { instanceDir });
    checkCancelled(profileId);
    await invoke('ensure_vanilla_version', { instanceDir, mcVersion: profile.mc_version });
    checkCancelled(profileId);

    let javaPath = "java";
    if (profile.java_version) {
        updateStatus(`Verificando Java ${profile.java_version}...`);
        updateCardProgress(profileId, 15, `Comprobando Java aislado...`);
        try {
            javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
        } catch (error) {
            checkCancelled(profileId);
            updateStatus(`Descargando Java aislado (${profile.java_version})...`);
            updateCardProgress(profileId, 25, `Descargando Java ${profile.java_version}...`);
            await invoke('download_java_command', { version: profile.java_version, baseDir });
            javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
        }
    } else {
        updateStatus("Usando Java del sistema...");
        updateCardProgress(profileId, 15, "Verificando instalación...");
    }
    checkCancelled(profileId);

    let isInstalled = false;
    try {
        isInstalled = await invoke('check_version_installed', { instanceDir, versionId: targetVersionId });
    } catch (e) {
        console.warn("No se pudo comprobar la versión", e);
    }

    if (profile.loader_name === 'Fabric') {
        if (!isInstalled || force) {
            updateStatus(`Preparando Fabric...`);
            updateCardProgress(profileId, 40, 'Preparando Fabric...');
            await invoke('ensure_fabric_profile', {
                instanceDir, mcVersion: profile.mc_version, loaderVersion: profile.loader_version
            });
        } else {
            updateStatus(`✔ Fabric ya estaba instalado.`);
            updateCardProgress(profileId, 40, `Verificado Fabric`);
        }
    } else if (profile.loader_url) {
    if (!isInstalled || force) {
        updateStatus(`Preparando ${profile.loader_name}...`);
        updateCardProgress(profileId, 40, `Instalando ${profile.loader_name}...`);
        const installerPath = `${installersDir}/${profile.loader_name.toLowerCase()}-${profile.mc_version}-installer.jar`;
        await invoke('download_generic_file', { url: profile.loader_url, destPath: installerPath });
        checkCancelled(profileId);
        await invoke('execute_jar', {
            javaPath, jarPath: installerPath,
            args: ["--installClient", instanceDir], workDir: installersDir
        });

        checkCancelled(profileId);
        const detectedId = await invoke('detect_installed_loader_version', {
            instanceDir, mcVersion: profile.mc_version
        });
        if (detectedId && detectedId !== profile.version_id) {
            profile.version_id = detectedId;
            await saveProfileToDisk(profileId, profile);
        }
    } else {
        updateStatus(`✔ ${profile.loader_name} ya estaba instalado.`);
        updateCardProgress(profileId, 40, `Verificado ${profile.loader_name}`);
    }
}
checkCancelled(profileId);

// usar el version_id correcto, ya sea el detectado o el original
const finalVersionId = profile.version_id || profile.mc_version;

return { javaPath, instanceDir, targetVersionId: finalVersionId, wasAlreadyInstalled: isInstalled };
}

export async function iniciarJuego(profileId, force = false, isLocal = false, localProfileData = null) {
    const profile = isLocal ? localProfileData : PROFILES[profileId];
    if (!profileId || !profile) return;

    if (!AUTH_SESSION) {
        updateStatus("Iniciá sesión antes de jugar");
        document.dispatchEvent(new CustomEvent('lumineria:require-login'));
        return;
    }
    let session = AUTH_SESSION;
    if (sessionNeedsRefresh()) {
        updateStatus("Renovando la sesión de Microsoft...");
        try {
            session = await ensureFreshSession();
        } catch (e) {
            console.warn('No se pudo renovar la sesión de Microsoft:', e);
            updateStatus("Tu sesión venció. Iniciá sesión de nuevo.");
            document.dispatchEvent(new CustomEvent('lumineria:require-login'));
            return;
        }
    }

    cancelRequested.delete(profileId);
    setCardPreparing(profileId, true);
    setInstancePreparing(profileId, true);
    updateCardProgress(profileId, 5, 'Preparando...');

    try {
        const { javaPath, instanceDir, targetVersionId } = await prepararCliente(profileId, profile, { force, isLocal });

        if (profile.packwiz_url && !profile.last_checked_at) {
            updateCardProgress(profileId, 60, 'Descargando mods por primera vez...');
            try {
                await sincronizarModpack(profileId, { silent: true });
                profile.last_checked_at = Date.now();
                await saveProfileToDisk(profileId, profile);
            } catch (e) {
                console.warn('No se pudo sincronizar mods, se continúa con los ya instalados:', e);
                updateStatus('Sin conexión al servidor de mods — iniciando con lo que ya está instalado.');
            }
        }
        checkCancelled(profileId);

        updateStatus("Descargando assets y lanzando el juego...");
        updateCardProgress(profileId, 85, 'Descargando assets...');

        const unlisten = await listen('assets-progress', (event) => {
            const { done, total } = event.payload;
            const pct = 85 + Math.floor((done / total) * 14);
            updateCardProgress(profileId, pct, `Descargando assets (${done}/${total})...`);
        });

        try {
            setInstanceRunning(profileId, true);
            await invoke('launch_minecraft', {
                options: {
                    profileId, title: profile.title, loaderName: profile.loader_name,
                    instanceDir, versionId: targetVersionId, javaPath,
                    ramMinMb: SETTINGS.ramMinMb, ramMaxMb: SETTINGS.ramMaxMb,
                    extraJavaArgs: SETTINGS.javaArgsExtra || ""
                },
                auth: session
            });
        } finally {
            unlisten();
        }

        updateCardProgress(profileId, 100, '¡Listo!');
        updateStatus("¡Disfruta tu aventura!");
        refreshCardStatus(profileId);

    } catch (e) {
        if (e?.isCancelled) {
            updateStatus("Descarga cancelada.");
            updateCardProgress(profileId, 0, '');
            setInstanceRunning(profileId, false);
            refreshCardStatus(profileId);
        } else {
            updateStatus(`Error: ${e}`);
            updateCardProgress(profileId, 0, '');
            console.error(e);
            setInstanceRunning(profileId, false);
            refreshCardStatus(profileId);
        }
    } finally {
        setCardPreparing(profileId, false);
        setInstancePreparing(profileId, false);
        setCardPlayState(profileId, false);
    }
}

export async function abrirCarpetaInstancia(profileId) {
    if (!profileId) return;
    const instanceDir = await getInstanceDir(profileId);
    await invoke('ensure_dir', { path: instanceDir });
    await invoke('open_folder', { path: instanceDir });
}
