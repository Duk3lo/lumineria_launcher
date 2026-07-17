import { PROFILES, getBaseDirectory, getInstanceDir, AUTH_SESSION, SETTINGS, resetInstanceLibraries } from './state.js';
import { updateStatus, updateCardProgress, setCardPlayState, refreshCardStatus } from './ui.js';
import { setInstanceRunning } from './instanceDetail.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export async function iniciarJuego(profileId, force = false) {
    if (!profileId || !PROFILES[profileId]) return;

    if (!AUTH_SESSION) {
        updateStatus("Iniciá sesión antes de jugar");
        document.dispatchEvent(new CustomEvent('lumineria:require-login'));
        return;
    }

    setCardPlayState(profileId, true);
    updateCardProgress(profileId, 5, 'Preparando...');

    const profile = PROFILES[profileId];
    const baseDir = await getBaseDirectory();
    const instanceDir = await getInstanceDir(profileId);
    const installersDir = `${baseDir}/installers`;
    const targetVersionId = profile.version_id || profile.mc_version;

    try {
        if (force) {
            updateStatus("Limpiando instalación anterior...");
            updateCardProgress(profileId, 2, 'Limpiando archivos previos...');
            await resetInstanceLibraries(profileId);
        }

        await invoke('ensure_dir', { path: instanceDir });
        await invoke('ensure_dir', { path: installersDir });
        await invoke('ensure_launcher_profile', { instanceDir });
        await invoke('ensure_vanilla_version', { instanceDir, mcVersion: profile.mc_version });
        let javaPath = "java";
        if (profile.java_version) {
            updateStatus(`Verificando Java ${profile.java_version}...`);
            updateCardProgress(profileId, 15, `Comprobando Java aislado...`);
            try {
                javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
            } catch (error) {
                updateStatus(`Descargando Java aislado (${profile.java_version})...`);
                updateCardProgress(profileId, 25, `Descargando Java ${profile.java_version}...`);
                await invoke('download_java_command', { version: profile.java_version, baseDir });
                javaPath = await invoke('verify_and_get_java', { version: profile.java_version, baseDir });
            }
        } else {
            updateStatus("Usando Java del sistema...");
            updateCardProgress(profileId, 15, "Verificando instalación...");
        }

        let isInstalled = false;
        try {
            isInstalled = await invoke('check_version_installed', { instanceDir, versionId: targetVersionId });
        } catch (e) {
            console.warn("No se pudo comprobar la versión", e);
        }

        if (profile.loader_url) {
            if (!isInstalled || force) {
                updateStatus(`Preparando ${profile.loader_name}...`);
                updateCardProgress(profileId, 40, `Instalando ${profile.loader_name}...`);

                const installerPath = `${installersDir}/${profile.loader_name.toLowerCase()}-${profile.mc_version}-installer.jar`;
                await invoke('download_generic_file', { url: profile.loader_url, destPath: installerPath });
                await invoke('execute_jar', {
                    javaPath,
                    jarPath: installerPath,
                    args: ["--installClient", instanceDir],
                    workDir: installersDir
                });
            } else {
                updateStatus(`✔ ${profile.loader_name} ya estaba instalado.`);
                updateCardProgress(profileId, 40, `Verificado ${profile.loader_name}`);
            }
        }

        if (profile.packwiz_url) {
            updateStatus(`Sincronizando mods de ${profile.title}...`);
            updateCardProgress(profileId, 60, 'Sincronizando mods...');

            const packwizUrl = "https://github.com/packwiz/packwiz-installer-bootstrap/releases/latest/download/packwiz-installer-bootstrap.jar";
            const packwizPath = `${installersDir}/packwiz-installer-bootstrap.jar`;

            await invoke('download_generic_file', { url: packwizUrl, destPath: packwizPath });
            await invoke('execute_jar', {
                javaPath,
                jarPath: packwizPath,
                args: [profile.packwiz_url],
                workDir: instanceDir
            });
        }

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
                    profileId: profileId,
                    instanceDir,
                    versionId: targetVersionId,
                    javaPath,
                    ramMinMb: SETTINGS.ramMinMb,
                    ramMaxMb: SETTINGS.ramMaxMb,
                    extraJavaArgs: SETTINGS.javaArgsExtra || ""
                },
                auth: AUTH_SESSION
            });
        } finally {
            unlisten();
        }

        updateCardProgress(profileId, 100, '¡Listo!');
        updateStatus("¡Disfruta tu aventura!");
        setTimeout(() => setCardPlayState(profileId, false), 1000);
        refreshCardStatus(profileId);

    } catch (e) {
        updateStatus(`Error: ${e}`);
        setCardPlayState(profileId, false);
        updateCardProgress(profileId, 0, '');
        console.error(e);
        setInstanceRunning(profileId, false);
    }
}

export async function abrirCarpetaInstancia(profileId) {
    if (!profileId) return;
    const instanceDir = await getInstanceDir(profileId);
    await invoke('ensure_dir', { path: instanceDir });
    await invoke('open_folder', { path: instanceDir });
}