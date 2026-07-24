import { PROFILES, deleteProfileFromDisk, syncSingleProfileFromDatabase, saveProfileToDisk } from '../../core/state.js';
import { invoke, listen } from '../../core/tauri.js';
import { iniciarJuego, abrirCarpetaInstancia, sincronizarModpack, isSyncing } from './launcher.js';
import { renderModsForInstance } from './mods.js';
import { renderResourcePacksForInstance } from './resourcePacks.js';
import { drawProfiles } from '../../ui/ui.js';
import { showAlert, showConfirm } from '../../ui/dialogs.js';

export let currentDetailProfileId = null;
let currentDetailIsLocal = false;
let currentDetailLocalProfile = null;

export const INSTANCE_STATE = {};
const autoSyncedThisSession = new Set();

const viewGrid = document.getElementById('view-grid');
const viewInstance = document.getElementById('view-instance');
const btnBack = document.getElementById('btn-back-grid');
const title = document.getElementById('instance-detail-title');
const btnPlayKill = document.getElementById('btn-instance-play');
const btnUpdate = document.getElementById('btn-instance-update');
const btnDelete = document.getElementById('btn-instance-delete');
const statusText = document.getElementById('instance-status-text');
const logsOutput = document.getElementById('instance-logs-output');
const btnCheckDb = document.getElementById('btn-instance-check-db');

let lastManualAction = 0;
const MANUAL_ACTION_COOLDOWN_MS = 4000;
const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

function debeAutoVerificar(profile) {
    if (!profile) return false;
    if (!profile.last_checked_at) return true;
    return (Date.now() - profile.last_checked_at) >= AUTO_CHECK_INTERVAL_MS;
}

async function marcarComprobado(profileId) {
    const profile = PROFILES[profileId];
    if (!profile) return;
    profile.last_checked_at = Date.now();
    await saveProfileToDisk(profileId, profile);
}

function puedeAccionar() {
    const now = Date.now();
    if (now - lastManualAction < MANUAL_ACTION_COOLDOWN_MS) return false;
    lastManualAction = now;
    return true;
}

function getCurrentProfile() {
    return currentDetailIsLocal ? currentDetailLocalProfile : PROFILES[currentDetailProfileId];
}

export function initInstanceDetail() {
    btnBack.addEventListener('click', () => {
        viewInstance.classList.add('hidden');
        viewGrid.classList.remove('hidden');
        currentDetailProfileId = null;
        currentDetailIsLocal = false;
        currentDetailLocalProfile = null;
    });

    btnPlayKill.addEventListener('click', async () => {
        if (!currentDetailProfileId) return;
        const state = INSTANCE_STATE[currentDetailProfileId];
        if (state && state.isRunning) {
            await invoke('kill_instance', { profileId: currentDetailProfileId });
            return;
        }
        if (btnPlayKill.dataset.mode === 'cancel-prep') {
            await invoke('cancel_preparation', { profileId: currentDetailProfileId });
            return;
        }
        iniciarJuego(currentDetailProfileId, false, currentDetailIsLocal, currentDetailLocalProfile);
    });

    btnUpdate?.addEventListener('click', async () => {
        if (!currentDetailProfileId || !puedeAccionar()) return;
        await runSyncForCurrentInstance();
    });

    document.getElementById('btn-open-folder-detail').addEventListener('click', () => {
        abrirCarpetaInstancia(currentDetailProfileId);
    });

    btnDelete?.addEventListener('click', async () => {
        if (!currentDetailProfileId) return;
        const id = currentDetailProfileId;
        const isLocal = currentDetailIsLocal;
        const profile = getCurrentProfile();
        const profileName = profile?.title || 'esta instancia';

        const confirmado = await showConfirm(
            isLocal
                ? `¿Eliminar "${profileName}" de tu carpeta .minecraft real? Esto borra la versión instalada directamente de tu instalación de Minecraft, no solo de este launcher.`
                : `¿Eliminar "${profileName}" permanentemente? Se borrará la carpeta de la instancia y no se podrá deshacer.`
        );
        if (!confirmado) return;

        try {
            if (isLocal) {
                await invoke('delete_vanilla_version', { versionId: id });
            } else {
                await deleteProfileFromDisk(id);
            }
            document.getElementById('btn-back-grid').click();
            drawProfiles();
        } catch (e) {
            await showAlert("Error al eliminar: " + e);
        }
    });

    const tabBtns = document.querySelectorAll('.tab-btn[data-tab]');
    const tabPanes = document.querySelectorAll('.tab-pane');

    tabBtns.forEach(btn => {
        btn.addEventListener('click', () => {
            tabBtns.forEach(b => b.classList.remove('active'));
            tabPanes.forEach(p => p.classList.add('hidden'));
            btn.classList.add('active');
            const targetId = btn.dataset.tab;
            document.getElementById(targetId).classList.remove('hidden');
            if (targetId === 'tab-mods' && currentDetailProfileId) {
                renderModsForInstance(currentDetailProfileId);
            }
            if (targetId === 'tab-resourcepacks' && currentDetailProfileId) {
                renderResourcePacksForInstance(currentDetailProfileId);
            }
        });
    });

    let logsFlushQueued = false;

    listen('game-started', (event) => {
        const { id } = event.payload;
        INSTANCE_STATE[id] = { isRunning: true, logs: [] };
        if (currentDetailProfileId === id) logsOutput.textContent = '';
        setInstanceRunning(id, true);
    });

    listen('game-log', (event) => {
        const { id, line } = event.payload;
        if (!INSTANCE_STATE[id]) INSTANCE_STATE[id] = { isRunning: true, logs: [] };

        const state = INSTANCE_STATE[id];
        state.logs.push(line);
        if (state.logs.length > 2000) state.logs.shift();

        if (currentDetailProfileId === id && !logsFlushQueued) {
            logsFlushQueued = true;
            requestAnimationFrame(() => {
                logsFlushQueued = false;
                logsOutput.textContent = INSTANCE_STATE[id].logs.join('\n') + '\n';
                logsOutput.scrollTop = logsOutput.scrollHeight;
            });
        }
    });

    listen('game-stopped', (event) => {
        const { id } = event.payload;
        if (INSTANCE_STATE[id]) {
            INSTANCE_STATE[id].isRunning = false;
        }
        if (currentDetailProfileId === id) {
            updatePlayKillButton(id);
        }
    });

    document.addEventListener('lumineria:sync-state-changed', (e) => {
        if (e.detail.id === currentDetailProfileId) {
            setBusyState(e.detail.syncing);
        }
    });


    btnCheckDb?.addEventListener('click', async () => {
        if (!currentDetailProfileId || !getCurrentProfile()?.is_official || !puedeAccionar()) return;

        setBusyState(true);
        statusText.innerText = "Consultando base de datos oficial...";

        try {
            const changed = await syncSingleProfileFromDatabase(currentDetailProfileId);
            await marcarComprobado(currentDetailProfileId);
            if (changed) {
                statusText.innerText = "¡Actualizado! Lista para jugar con la nueva versión.";
                drawProfiles();
            } else {
                statusText.innerText = "Todo actualizado: Lista para jugar.";
            }
        } catch (e) {
            statusText.innerText = `${e.message}`;
        } finally {
            setTimeout(() => updatePlayKillButton(currentDetailProfileId), 4000);
            setBusyState(false);
        }
    });
}

export function setInstancePreparing(profileId, isPreparing) {
    if (currentDetailProfileId !== profileId) return;
    if (isPreparing) {
        btnPlayKill.innerText = "Cancelar preparación";
        btnPlayKill.classList.add('btn-kill');
        btnPlayKill.dataset.mode = 'cancel-prep';
        statusText.innerText = "Preparando instancia...";
    } else {
        btnPlayKill.dataset.mode = '';
        updatePlayKillButton(profileId);
    }
}

export function openInstanceDetail(profileId, isLocal = false, localProfile = null) {
    currentDetailProfileId = profileId;
    currentDetailIsLocal = isLocal;
    currentDetailLocalProfile = localProfile;
    const profile = getCurrentProfile();

    if (!INSTANCE_STATE[profileId]) {
        INSTANCE_STATE[profileId] = { isRunning: false, logs: [] };
    }

    title.innerText = profile?.title || profileId;
    logsOutput.textContent = INSTANCE_STATE[profileId].logs.join('\n') + (INSTANCE_STATE[profileId].logs.length > 0 ? '\n' : '');
    logsOutput.scrollTop = logsOutput.scrollHeight;

    const hasPackwiz = !!profile?.packwiz_url;
    if (btnUpdate) btnUpdate.classList.toggle('hidden', !hasPackwiz);

    const isOfficial = profile?.is_official === true;
    if (btnCheckDb) btnCheckDb.classList.toggle('hidden', !isOfficial);

    updatePlayKillButton(profileId);

    const defaultTab = document.querySelector('.tab-btn[data-tab="tab-logs"]');
    if (defaultTab) defaultTab.click();

    viewGrid.classList.add('hidden');
    viewInstance.classList.remove('hidden');

    if (!isLocal && (hasPackwiz || isOfficial) && !isSyncing(profileId) && debeAutoVerificar(profile)) {
        runSyncForCurrentInstance({ silent: true });
    }
}

export function setInstanceRunning(profileId, isRunning) {
    if (!INSTANCE_STATE[profileId]) INSTANCE_STATE[profileId] = { isRunning: false, logs: [] };
    INSTANCE_STATE[profileId].isRunning = isRunning;
    if (currentDetailProfileId === profileId) {
        if (isRunning) {
            btnPlayKill.dataset.mode = '';
        }
        updatePlayKillButton(profileId);
    }
}

async function runSyncForCurrentInstance({ silent = false } = {}) {
    const profileId = currentDetailProfileId;
    if (!profileId) return;

    const profile = getCurrentProfile();
    if (!profile) return;

    setBusyState(true);
    if (!silent) statusText.innerText = "Comprobando actualizaciones...";

    try {
        let changed = false;

        if (profile.is_official) {
            if (!silent) statusText.innerText = "Comprobando cliente en la base de datos...";
            try { changed = await syncSingleProfileFromDatabase(profileId); } catch (e) { }
        }

        if (profile.packwiz_url) {
            if (!silent) statusText.innerText = "Sincronizando mods...";
            await sincronizarModpack(profileId, { silent: true });
        }

        await marcarComprobado(profileId);

        if (currentDetailProfileId === profileId) {
            if (changed) {
                statusText.innerText = "Instancia y mods actualizados a la última versión.";
                drawProfiles();
            } else {
                statusText.innerText = "Todo está actualizado y listo.";
            }
        }
    } catch (e) {
        console.warn('Error al actualizar la instancia:', e);
        if (currentDetailProfileId === profileId) {
            if (e?.isConnectionError) {
                statusText.innerText = "Sin conexión a la base de datos de mods.";
            } else if (!silent) {
                statusText.innerText = `Error al actualizar: ${e.message || e}`;
            }
        }
    } finally {
        setBusyState(false);
        if (currentDetailProfileId === profileId) {
            setTimeout(() => updatePlayKillButton(profileId), 3000);
        }
    }
}

function setBusyState(busy) {
    if (btnPlayKill) btnPlayKill.disabled = busy;
    if (btnDelete) btnDelete.disabled = busy;
    if (btnCheckDb) btnCheckDb.disabled = busy;
    if (btnUpdate) {
        btnUpdate.disabled = busy;
        btnUpdate.innerText = busy ? '⏳ Actualizando...' : '🔄 Actualizar paquetes';
    }
}

function updatePlayKillButton(profileId) {
    const isRunning = INSTANCE_STATE[profileId]?.isRunning || false;
    if (isRunning) {
        btnPlayKill.innerText = "Detener / Kill";
        btnPlayKill.classList.add('btn-kill');
        statusText.innerText = "El juego se está ejecutando...";
    } else {
        btnPlayKill.innerText = "Jugar";
        btnPlayKill.classList.remove('btn-kill');
        statusText.innerText = "Lista para jugar";
    }
}
