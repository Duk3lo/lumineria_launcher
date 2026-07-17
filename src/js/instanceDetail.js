import { PROFILES } from './state.js';
import { iniciarJuego, abrirCarpetaInstancia } from './launcher.js';
import { renderModsForInstance } from './mods.js';
import { renderResourcePacksForInstance } from './resourcePacks.js';
import { deleteProfileFromDisk } from './state.js';
import { drawProfiles } from './ui.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

export let currentDetailProfileId = null;

export const INSTANCE_STATE = {};

const viewGrid = document.getElementById('view-grid');
const viewInstance = document.getElementById('view-instance');
const btnBack = document.getElementById('btn-back-grid');
const title = document.getElementById('instance-detail-title');
const btnPlayKill = document.getElementById('btn-instance-play');
const statusText = document.getElementById('instance-status-text');
const logsOutput = document.getElementById('instance-logs-output');

export function initInstanceDetail() {
    btnBack.addEventListener('click', () => {
        viewInstance.classList.add('hidden');
        viewGrid.classList.remove('hidden');
        currentDetailProfileId = null;
    });

    btnPlayKill.addEventListener('click', async () => {
        if (!currentDetailProfileId) return;
        const state = INSTANCE_STATE[currentDetailProfileId];
        if (state && state.isRunning) {
            await invoke('kill_instance', { profileId: currentDetailProfileId });
        } else {
            iniciarJuego(currentDetailProfileId);
        }
    });

    document.getElementById('btn-open-folder-detail').addEventListener('click', () => {
        abrirCarpetaInstancia(currentDetailProfileId);
    });

    document.getElementById('btn-instance-delete')?.addEventListener('click', async () => {
        if (!currentDetailProfileId) return;
        const id = currentDetailProfileId;
        if (confirm("¿Eliminar esta instancia permanentemente?")) {
            await deleteProfileFromDisk(id);
            document.getElementById('btn-back-grid').click();
            drawProfiles();
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
}

export function openInstanceDetail(profileId) {
    currentDetailProfileId = profileId;
    const profile = PROFILES[profileId];

    if (!INSTANCE_STATE[profileId]) {
        INSTANCE_STATE[profileId] = { isRunning: false, logs: [] };
    }

    title.innerText = profile.title;
    logsOutput.textContent = INSTANCE_STATE[profileId].logs.join('\n') + (INSTANCE_STATE[profileId].logs.length > 0 ? '\n' : '');
    logsOutput.scrollTop = logsOutput.scrollHeight;

    updatePlayKillButton(profileId);

    const defaultTab = document.querySelector('.tab-btn[data-tab="tab-logs"]');
    if (defaultTab) defaultTab.click();

    viewGrid.classList.add('hidden');
    viewInstance.classList.remove('hidden');
}

export function setInstanceRunning(profileId, isRunning) {
    if (!INSTANCE_STATE[profileId]) INSTANCE_STATE[profileId] = { isRunning: false, logs: [] };
    INSTANCE_STATE[profileId].isRunning = isRunning;

    if (currentDetailProfileId === profileId) {
        updatePlayKillButton(profileId);
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