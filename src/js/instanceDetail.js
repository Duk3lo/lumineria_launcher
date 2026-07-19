import { PROFILES, deleteProfileFromDisk } from './state.js';
import { iniciarJuego, abrirCarpetaInstancia, sincronizarModpack, isSyncing } from './launcher.js';
import { renderModsForInstance } from './mods.js';
import { renderResourcePacksForInstance } from './resourcePacks.js';
import { drawProfiles } from './ui.js';
import { showAlert, showConfirm } from './dialogs.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export let currentDetailProfileId = null;
let currentDetailIsLocal = false;
let currentDetailLocalProfile = null;

export const INSTANCE_STATE = {};

const viewGrid = document.getElementById('view-grid');
const viewInstance = document.getElementById('view-instance');
const btnBack = document.getElementById('btn-back-grid');
const title = document.getElementById('instance-detail-title');
const btnPlayKill = document.getElementById('btn-instance-play');
const btnUpdate = document.getElementById('btn-instance-update');
const btnDelete = document.getElementById('btn-instance-delete');
const statusText = document.getElementById('instance-status-text');
const logsOutput = document.getElementById('instance-logs-output');

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
        if (!currentDetailProfileId) return;
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

    listen('game-log', (event) => {
        const { id, line } = event.payload;
        if (!INSTANCE_STATE[id]) INSTANCE_STATE[id] = { isRunning: true, logs: [] };

        INSTANCE_STATE[id].logs.push(line);
        if (INSTANCE_STATE[id].logs.length > 2000) INSTANCE_STATE[id].logs.shift();
        if (currentDetailProfileId === id) {
            logsOutput.textContent += line + '\n';
            logsOutput.scrollTop = logsOutput.scrollHeight;
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
}

listen('game-started', (event) => {
    setInstanceRunning(event.payload.id, true);
});

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

    updatePlayKillButton(profileId);

    const defaultTab = document.querySelector('.tab-btn[data-tab="tab-logs"]');
    if (defaultTab) defaultTab.click();

    viewGrid.classList.add('hidden');
    viewInstance.classList.remove('hidden');

    if (hasPackwiz && !isSyncing(profileId)) {
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

    setBusyState(true);
    if (!silent) statusText.innerText = "Sincronizando mods...";

    try {
        await sincronizarModpack(profileId, { silent: true });
        if (currentDetailProfileId === profileId) {
            statusText.innerText = "Mods actualizados.";
        }
    } catch (e) {
        console.warn('No se pudo sincronizar el modpack:', e);
        if (currentDetailProfileId === profileId) {
            statusText.innerText = "Error al actualizar mods.";
        }
    } finally {
        setBusyState(false);
        if (currentDetailProfileId === profileId) {
            updatePlayKillButton(profileId);
        }
    }
}

function setBusyState(busy) {
    if (btnPlayKill) btnPlayKill.disabled = busy;
    if (btnDelete) btnDelete.disabled = busy;
    if (btnUpdate) {
        btnUpdate.disabled = busy;
        btnUpdate.innerText = busy ? '⏳ Actualizando...' : '🔄 Actualizar todo';
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