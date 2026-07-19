import {
    PROFILES,
    setProfileSelection,
    SETTINGS,
    loadSettings,
    saveSettings,
    getSystemRamMb,
    getInstanceStatus,
    deleteProfileFromDisk
} from './state.js';

import { showAlert, showConfirm } from './dialogs.js';

const { invoke } = window.__TAURI__.core;
const statusText = document.getElementById('status-text');
const profilesGrid = document.getElementById('profiles-grid');

const { listen } = window.__TAURI__.event;
const runningInstances = new Set();

export function isInstanceRunning(id) {
    return runningInstances.has(id);
}

export function setCardRunningState(id, isRunning) {
    if (isRunning) runningInstances.add(id);
    else runningInstances.delete(id);

    const playBtn = document.getElementById(`play-btn-${id}`);
    if (!playBtn) return;
    playBtn.innerText = isRunning ? 'Detener' : 'Jugar';
    playBtn.classList.toggle('btn-kill', isRunning);
    playBtn.dataset.action = isRunning ? 'kill' : 'play';
}

export function initInstanceEventListeners() {
    listen('game-started', (event) => setCardRunningState(event.payload.id, true));
    listen('game-stopped', (event) => {
        setCardRunningState(event.payload.id, false);
        refreshCardStatus(event.payload.id);
    });
}

export function updateStatus(text) {
    if (statusText) statusText.innerText = text;
}

export async function drawProfiles() {
    if (!profilesGrid) return;
    profilesGrid.innerHTML = '';

    if (Object.keys(PROFILES).length > 0) {
        renderSection("Mis Instancias Personalizadas", PROFILES, false);
    }
    try {
        const vanillaLocales = await invoke('get_installed_vanilla_versions');
        if (Object.keys(vanillaLocales).length > 0) {
            renderSection("Detectado en .minecraft (PC)", vanillaLocales, true);
        }
    } catch (e) {
        console.warn("No se pudieron buscar versiones de .minecraft:", e);
    }
    if (profilesGrid.innerHTML === '') {
        profilesGrid.innerHTML = `<p class="mods-empty-state">No hay instancias. ¡Crea una nueva o instala una oficial!</p>`;
    }
    refreshAllCardStatuses(Object.keys(PROFILES));
}
function renderSection(title, items, isVanillaLocal) {
    const header = document.createElement('h2');
    header.className = 'section-title';
    header.innerText = title;
    header.style = "grid-column: 1 / -1; margin: 30px 0 15px 0; font-size: 1.1rem; color: var(--primary-glow); border-left: 4px solid var(--primary-glow); padding-left: 15px; background: rgba(192, 132, 252, 0.05); padding-top: 5px; padding-bottom: 5px; border-radius: 0 8px 8px 0;";
    profilesGrid.appendChild(header);

    Object.keys(items).forEach(id => {
        profilesGrid.appendChild(buildProfileCard(id, items[id], isVanillaLocal));
    });
}

function buildProfileCard(id, profile, isVanillaLocal) {
    const card = document.createElement('div');
    card.className = 'profile-card';
    if (isVanillaLocal) card.classList.add('local-pc-card');
    card.id = `card-${id}`;
    card.dataset.profileId = id;

    const imageUrl = profile.image ? profile.image : 'assets/logo.png';

    card.innerHTML = `
        <div class="profile-card-bg" style="background-image:url('${imageUrl}')"></div>
        <div class="profile-content">
            <div class="profile-title-row">
                <h3 class="profile-title">${profile.title}</h3>
                <span class="status-dot" id="status-dot-${id}" title="Comprobando..."></span>
            </div>
            <div class="profile-badges">
                <span class="badge loader">${profile.loader_name} ${isVanillaLocal ? '(PC)' : ''}</span>
                <span class="badge version">${profile.mc_version}</span>
            </div>

            <div class="card-progress hidden" id="card-progress-${id}">
                <div class="card-progress-bar"><div class="card-progress-fill" id="card-progress-fill-${id}"></div></div>
                <span class="card-progress-label" id="card-progress-label-${id}">Preparando...</span>
            </div>

            <div class="profile-actions">
                <div class="play-button-group">
                    <button class="play-btn-card" id="play-btn-${id}" data-action="play">Jugar</button>
                    ${!isVanillaLocal ? `
<button class="play-dropdown-toggle" id="dropdown-toggle-${id}" data-action="toggle-menu">⋮</button>
<div class="card-dropdown-menu hidden" id="dropdown-menu-${id}">
    <button data-action="open-folder">📂 Abrir Carpeta</button>
    <button data-action="view-mods">🧩 Ver Mods</button>
    <button data-action="reinstall">🔄 Reinstalar</button>
    <hr style="border: 0; border-top: 1px solid rgba(255,255,255,0.1); margin: 4px 0;">
    <button data-action="delete" style="color: #f87171;">🗑 Eliminar Instancia</button>
</div>
` : `

<button class="play-dropdown-toggle" id="dropdown-toggle-${id}" data-action="toggle-menu">⋮</button>
<div class="card-dropdown-menu hidden" id="dropdown-menu-${id}">
    <button data-action="delete-local" style="color: #f87171;">🗑 Eliminar de .minecraft</button>
</div>
`}
                </div>
            </div>
        </div>
    `;

    card.addEventListener('click', async (event) => {
        const btn = event.target.closest('button');
        const action = btn ? btn.dataset.action : null;
        if (action === 'toggle-menu') {
            event.stopPropagation();
            toggleCardDropdown(id);
            return;
        }
        if (action === 'play') {
            event.stopPropagation();
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:play-profile', {
                detail: { id, isLocal: isVanillaLocal, localProfile: isVanillaLocal ? profile : null }
            }));
            return;
        }
        if (action === 'kill') {
            event.stopPropagation();
            invoke('kill_instance', { profileId: id });
            return;
        }
        if (action === 'delete') {
            event.stopPropagation();
            closeAllDropdowns();
            const confirmado = await showConfirm(
                `¿Eliminar "${profile.title}" permanentemente? Se borrará la carpeta de la instancia y no se podrá deshacer.`
            );
            if (confirmado) {
                try {
                    await deleteProfileFromDisk(id);
                    updateStatus(`Instancia "${profile.title}" eliminada.`);
                    drawProfiles();
                } catch (e) {
                    await showAlert("Error al eliminar: " + e);
                }
            }
            return;
        }
        closeAllDropdowns();
        document.dispatchEvent(new CustomEvent('lumineria:open-instance-detail', {
            detail: { id, isLocal: isVanillaLocal, localProfile: isVanillaLocal ? profile : null }
        }));
    });

    return card;
}

function toggleCardDropdown(id) {
    const menu = document.getElementById(`dropdown-menu-${id}`);
    if (!menu) return;
    const isOpen = !menu.classList.contains('hidden');
    closeAllDropdowns();
    if (!isOpen) menu.classList.remove('hidden');
}

export function closeAllDropdowns() {
    document.querySelectorAll('.card-dropdown-menu').forEach(menu => menu.classList.add('hidden'));
}

async function refreshAllCardStatuses(profileKeys) {
    for (const id of profileKeys) {
        refreshCardStatus(id);
    }
}

export async function refreshCardStatus(id) {
    try {
        const status = await getInstanceStatus(id);
        const dot = document.getElementById(`status-dot-${id}`);
        const playBtn = document.getElementById(`play-btn-${id}`);

        if (dot) dot.classList.toggle('installed', status.installed);
        if (playBtn && !isInstanceRunning(id)) {
            playBtn.innerText = status.installed ? 'Jugar' : 'Instalar';
        }
    } catch (e) { }
}

export function setCardPlayState(id, disabled) {
    const playBtn = document.getElementById('play-btn-' + id);
    if (playBtn) playBtn.disabled = disabled;
}

export function updateCardProgress(id, percent, label) {
    const container = document.getElementById(`card-progress-${id}`);
    const fill = document.getElementById(`card-progress-fill-${id}`);
    const labelEl = document.getElementById(`card-progress-label-${id}`);
    if (!container || !fill) return;

    if (percent <= 0) {
        container.classList.add('hidden');
        fill.style.width = '0%';
        return;
    }

    container.classList.remove('hidden');
    fill.style.width = `${percent}%`;
    if (labelEl && label) labelEl.innerText = label;

    if (percent >= 100) setTimeout(() => container.classList.add('hidden'), 1500);
}

export async function initSettingsPanel() {
    await loadSettings();
    const ramMinInput = document.getElementById('ram-min-input');
    const ramMaxInput = document.getElementById('ram-max-input');
    const javaArgsInput = document.getElementById('java-args-input');
    const saveBtn = document.getElementById('settings-save-btn');

    if (!ramMinInput || !ramMaxInput) return;

    ramMinInput.value = SETTINGS.ramMinMb;
    ramMaxInput.value = SETTINGS.ramMaxMb;
    if (javaArgsInput) javaArgsInput.value = SETTINGS.javaArgsExtra || "";

    saveBtn?.addEventListener('click', async () => {
        const min = parseInt(ramMinInput.value);
        const max = parseInt(ramMaxInput.value);

        if (min > max) return showAlert("La RAM mínima no puede ser mayor a la máxima.");

        await saveSettings({
            ramMinMb: min,
            ramMaxMb: max,
            javaArgsExtra: javaArgsInput.value
        });
        updateStatus("Ajustes guardados correctamente.");
    });
}