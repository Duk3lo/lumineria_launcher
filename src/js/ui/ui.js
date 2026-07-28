import {
    PROFILES,
    setProfileSelection,
    SETTINGS,
    loadSettings,
    saveSettings,
    getSystemRamMb,
    getInstanceStatus,
    deleteProfileFromDisk
} from '../core/state.js';
import { invoke, listen } from '../core/tauri.js';
import { setSafeBackgroundImage } from '../core/domSafety.js';

import { showAlert, showConfirm } from './dialogs.js';

const statusText = document.getElementById('status-text');
const profilesGrid = document.getElementById('profiles-grid');

const runningInstances = new Set();
const cardProgressState = new Map();

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

let currentViewType = 'custom';
export async function drawProfiles(type = null) {
    if (type) currentViewType = type;
    else type = currentViewType;

    if (!profilesGrid) return;
    closeAllDropdowns();
    profilesGrid.innerHTML = '';
    
    const skeleton = document.createElement('div');
    skeleton.className = 'profiles-loading';
    skeleton.innerHTML = `
        <div class="skeleton-card"></div>
        <div class="skeleton-card"></div>
        <div class="skeleton-card"></div>
    `;
    profilesGrid.appendChild(skeleton);

    let profilesToRender = [];

    if (type === 'custom') {
        profilesGrid.innerHTML = '';
        if (Object.keys(PROFILES).length > 0) {
            renderSection("Mis Instancias Personalizadas", PROFILES, false);
            profilesToRender = Object.keys(PROFILES);
        }
    } else if (type === 'local') {
        let vanillaLocales = {};
        try {
            vanillaLocales = await invoke('get_installed_vanilla_versions');
        } catch (e) {
            console.warn("No se pudieron buscar versiones de .minecraft:", e);
        }

        profilesGrid.innerHTML = '';
        if (Object.keys(vanillaLocales).length > 0) {
            renderSection("Detectado en .minecraft (PC)", vanillaLocales, true);
            profilesToRender = Object.keys(vanillaLocales);
        }
    }

    if (profilesGrid.innerHTML === '') {
        const emptyMsg = document.createElement('p');
        emptyMsg.className = 'mods-empty-state';
        emptyMsg.textContent = type === 'custom' 
            ? 'No hay instancias. ¡Crea una nueva o instala una oficial!' 
            : 'No se detectaron instancias en tu carpeta .minecraft.';
        profilesGrid.appendChild(emptyMsg);
    }
    
    refreshAllCardStatuses(profilesToRender);
    profilesToRender.forEach(id => applyCardProgressDOM(id));
}

function renderSection(title, items, isVanillaLocal) {
    const header = document.createElement('h2');
    header.className = 'section-title';
    header.textContent = title;
    header.style.cssText = "grid-column: 1 / -1; margin: 30px 0 15px 0; font-size: 1.1rem; color: var(--primary-glow); border-left: 4px solid var(--primary-glow); padding-left: 15px; background: rgba(192, 132, 252, 0.05); padding-top: 5px; padding-bottom: 5px; border-radius: 0 8px 8px 0;";
    profilesGrid.appendChild(header);

    Object.keys(items).forEach(id => {
        profilesGrid.appendChild(buildProfileCard(id, items[id], isVanillaLocal));
    });
}

function buildProfileCard(id, profile, isVanillaLocal) {
    const imageUrl = profile.image ? profile.image : 'assets/logo.png';
    const dotClass = isVanillaLocal ? "status-dot installed" : "status-dot";
    const dotTitle = isVanillaLocal ? "Instalado (Local)" : "Comprobando...";

    const card = document.createElement('div');
    card.className = 'profile-card';
    if (isVanillaLocal) card.classList.add('local-pc-card');
    card.id = `card-${id}`;
    card.dataset.profileId = id;

    const bg = document.createElement('div');
    bg.className = 'profile-card-bg';
    setSafeBackgroundImage(bg, imageUrl);

    const content = document.createElement('div');
    content.className = 'profile-content';

    const titleRow = document.createElement('div');
    titleRow.className = 'profile-title-row';

    const h3 = document.createElement('h3');
    h3.className = 'profile-title';
    h3.textContent = profile.title;

    const dot = document.createElement('span');
    dot.className = dotClass;
    dot.id = `status-dot-${id}`;
    dot.title = dotTitle;

    titleRow.append(h3, dot);

    const badges = document.createElement('div');
    badges.className = 'profile-badges';

    const loaderBadge = document.createElement('span');
    loaderBadge.className = 'badge loader';
    loaderBadge.textContent = `${profile.loader_name}${isVanillaLocal ? ' (PC)' : ''}`;

    const versionBadge = document.createElement('span');
    versionBadge.className = 'badge version';
    versionBadge.textContent = profile.mc_version;

    badges.append(loaderBadge, versionBadge);

    const progress = document.createElement('div');
    progress.className = 'card-progress hidden';
    progress.id = `card-progress-${id}`;

    const progressBar = document.createElement('div');
    progressBar.className = 'card-progress-bar';
    const progressFill = document.createElement('div');
    progressFill.className = 'card-progress-fill';
    progressFill.id = `card-progress-fill-${id}`;
    progressBar.appendChild(progressFill);

    const progressLabel = document.createElement('span');
    progressLabel.className = 'card-progress-label';
    progressLabel.id = `card-progress-label-${id}`;
    progressLabel.textContent = 'Preparando...';

    progress.append(progressBar, progressLabel);

    const actions = document.createElement('div');
    actions.className = 'profile-actions';
    const playGroup = document.createElement('div');
    playGroup.className = 'play-button-group';

    const playBtn = document.createElement('button');
    playBtn.className = 'play-btn-card';
    playBtn.id = `play-btn-${id}`;
    playBtn.dataset.action = 'play';
    playBtn.textContent = 'Jugar';
    playGroup.appendChild(playBtn);

    const dropdownToggle = document.createElement('button');
    dropdownToggle.className = 'play-dropdown-toggle';
    dropdownToggle.id = `dropdown-toggle-${id}`;
    dropdownToggle.dataset.action = 'toggle-menu';
    dropdownToggle.textContent = '⋮';
    playGroup.appendChild(dropdownToggle);

    const dropdownMenu = document.createElement('div');
    dropdownMenu.className = 'card-dropdown-menu hidden';
    dropdownMenu.id = `dropdown-menu-${id}`;

    if (!isVanillaLocal) {
        [
            { action: 'open-folder', text: '📂 Abrir Carpeta' },
            { action: 'view-mods', text: '🧩 Ver Mods' },
            { action: 'reinstall', text: '🔄 Reinstalar' },
        ].forEach(({ action, text }) => {
            const btn = document.createElement('button');
            btn.dataset.action = action;
            btn.textContent = text;
            dropdownMenu.appendChild(btn);
        });

        const hr = document.createElement('hr');
        hr.style.cssText = 'border: 0; border-top: 1px solid rgba(255,255,255,0.1); margin: 4px 0;';
        dropdownMenu.appendChild(hr);

        const deleteBtn = document.createElement('button');
        deleteBtn.dataset.action = 'delete';
        deleteBtn.style.color = '#f87171';
        deleteBtn.textContent = '🗑 Eliminar Instancia';
        dropdownMenu.appendChild(deleteBtn);
    } else {
        const deleteLocalBtn = document.createElement('button');
        deleteLocalBtn.dataset.action = 'delete-local';
        deleteLocalBtn.style.color = '#f87171';
        deleteLocalBtn.textContent = '🗑 Eliminar de .minecraft';
        dropdownMenu.appendChild(deleteLocalBtn);
    }

    playGroup.appendChild(dropdownMenu);
    dropdownMenu._originalParent = playGroup;

    dropdownMenu.addEventListener('click', async (event) => {
        const btn = event.target.closest('button');
        if (!btn) return;
        const action = btn.dataset.action;

        if (action === 'open-folder') {
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:open-folder', { detail: { id } }));
            return;
        }
        if (action === 'view-mods') {
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:open-mods', { detail: { id } }));
            return;
        }
        if (action === 'reinstall') {
            closeAllDropdowns();
            const confirmado = await showConfirm(`¿Reinstalar "${profile.title}"? Se reinstalará el cargador y las librerías.`);
            if (confirmado) {
                document.dispatchEvent(new CustomEvent('lumineria:play-profile', {
                    detail: { id, force: true, isLocal: false, localProfile: null }
                }));
            }
            return;
        }
        if (action === 'delete') {
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
        if (action === 'delete-local') {
            closeAllDropdowns();
            const confirmado = await showConfirm(
                `¿Eliminar "${profile.title}" de tu carpeta .minecraft real? Esto borra la versión instalada directamente de tu instalación de Minecraft.`
            );
            if (confirmado) {
                try {
                    await invoke('delete_vanilla_version', { versionId: id });
                    updateStatus(`"${profile.title}" eliminado de .minecraft.`);
                    drawProfiles();
                } catch (e) {
                    await showAlert("Error al eliminar: " + e);
                }
            }
            return;
        }
    });

    actions.appendChild(playGroup);

    content.append(titleRow, badges, progress, actions);
    card.append(bg, content);

    card.addEventListener('click', async (event) => {
        const btn = event.target.closest('button');
        const action = btn ? btn.dataset.action : null;

        if (btn) event.stopPropagation();

        if (action === 'toggle-menu') {
            toggleCardDropdown(id);
            return;
        }
        if (action === 'play') {
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:play-profile', {
                detail: { id, isLocal: isVanillaLocal, localProfile: isVanillaLocal ? profile : null }
            }));
            return;
        }
        if (action === 'kill') {
            invoke('kill_instance', { profileId: id });
            return;
        }

        if (action === 'cancel-prep') {
            btn.innerText = 'Cancelando...';
            btn.disabled = true;
            document.dispatchEvent(new CustomEvent('lumineria:cancel-preparation', { detail: { id } }));
            return;
        }

        if (action === 'open-folder') {
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:open-folder', { detail: { id } }));
            return;
        }
        if (action === 'view-mods') {
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:open-mods', { detail: { id } }));
            return;
        }
        if (action === 'reinstall') {
            closeAllDropdowns();
            const confirmado = await showConfirm(`¿Reinstalar "${profile.title}"? Se reinstalará el cargador y las librerías.`);
            if (confirmado) {
                document.dispatchEvent(new CustomEvent('lumineria:play-profile', {
                    detail: { id, force: true, isLocal: false, localProfile: null }
                }));
            }
            return;
        }
        if (action === 'delete') {
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
        if (action === 'delete-local') {
            closeAllDropdowns();
            const confirmado = await showConfirm(
                `¿Eliminar "${profile.title}" de tu carpeta .minecraft real? Esto borra la versión instalada directamente de tu instalación de Minecraft.`
            );
            if (confirmado) {
                try {
                    await invoke('delete_vanilla_version', { versionId: id });
                    updateStatus(`"${profile.title}" eliminado de .minecraft.`);
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
    const toggleBtn = document.getElementById(`dropdown-toggle-${id}`);
    if (!menu || !toggleBtn) return;

    const isOpen = !menu.classList.contains('hidden');
    closeAllDropdowns();
    if (isOpen) return;

    document.body.appendChild(menu);
    menu.classList.remove('hidden');
    positionDropdown(menu, toggleBtn);
}

function positionDropdown(menu, anchorEl) {
    menu.style.position = 'fixed';
    menu.style.zIndex = '9999';
    menu.style.visibility = 'hidden';
    menu.style.top = '0px';
    menu.style.left = '0px';
    menu.style.right = 'auto';
    menu.style.bottom = 'auto';

    const anchorRect = anchorEl.getBoundingClientRect();
    const menuRect = menu.getBoundingClientRect();
    const margin = 6;

    let top = anchorRect.bottom + margin;
    if (top + menuRect.height > window.innerHeight - margin) {
        top = anchorRect.top - menuRect.height - margin;
        if (top < margin) top = margin;
    }

    let left = anchorRect.right - menuRect.width;
    if (left < margin) left = margin;
    if (left + menuRect.width > window.innerWidth - margin) {
        left = window.innerWidth - menuRect.width - margin;
    }

    menu.style.top = `${top}px`;
    menu.style.left = `${left}px`;
    menu.style.visibility = 'visible';
}

export function closeAllDropdowns() {
    document.querySelectorAll('.card-dropdown-menu').forEach(menu => {
        menu.classList.add('hidden');
        if (menu._originalParent && menu.parentElement !== menu._originalParent) {
            menu._originalParent.appendChild(menu);
        }
    });
}
document.addEventListener('click', (e) => {
    if (!e.target.closest('.card-dropdown-menu') && !e.target.closest('[data-action="toggle-menu"]')) {
        closeAllDropdowns();
    }
});
document.addEventListener('scroll', closeAllDropdowns, true);

async function refreshAllCardStatuses(profileKeys) {
    for (const id of profileKeys) {
        refreshCardStatus(id);
    }
}

export async function refreshCardStatus(id) {
    try {
        const dot = document.getElementById(`status-dot-${id}`);
        const playBtn = document.getElementById(`play-btn-${id}`);
        const card = document.getElementById(`card-${id}`);
        const isLocal = card && card.classList.contains('local-pc-card');

        if (isLocal) {
            if (dot) {
                dot.classList.add('installed');
                dot.title = 'Instalado (Local)';
            }
            if (playBtn && !isInstanceRunning(id)) {
                playBtn.innerText = 'Jugar';
            }
            return;
        }
        const status = await getInstanceStatus(id);

        if (dot) {
            dot.classList.toggle('installed', status.installed);
            dot.title = status.installed ? 'Instalado y listo' : 'No instalado';
        }
        if (playBtn && !isInstanceRunning(id)) {
            playBtn.innerText = status.installed ? 'Jugar' : 'Instalar';
        }
    } catch (e) {
        console.warn(`No se pudo comprobar el estado de ${id}:`, e);
    }
}

export function setCardPlayState(id, disabled) {
    const playBtn = document.getElementById('play-btn-' + id);
    if (playBtn) playBtn.disabled = disabled;
}

export function updateCardProgress(id, percent, label) {
    if (percent <= 0) {
        cardProgressState.delete(id);
    } else {
        cardProgressState.set(id, { percent, label });
    }
    applyCardProgressDOM(id);
}

function applyCardProgressDOM(id) {
    const container = document.getElementById(`card-progress-${id}`);
    const fill = document.getElementById(`card-progress-fill-${id}`);
    const labelEl = document.getElementById(`card-progress-label-${id}`);
    if (!container || !fill) return;

    const state = cardProgressState.get(id);
    if (!state) {
        container.classList.add('hidden');
        fill.style.width = '0%';
        return;
    }

    container.classList.remove('hidden');
    fill.style.width = `${state.percent}%`;
    if (labelEl && state.label) labelEl.innerText = state.label;

    if (state.percent >= 100) {
        setTimeout(() => {
            container.classList.add('hidden');
            cardProgressState.delete(id);
        }, 1500);
    }
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

export function setCardPreparing(id, isPreparing) {
    const playBtn = document.getElementById(`play-btn-${id}`);
    if (!playBtn) return;

    if (isPreparing) {
        playBtn.innerText = 'Cancelar';
        playBtn.classList.add('btn-kill');
        playBtn.dataset.action = 'cancel-prep';
        playBtn.disabled = false;
    } else if (isInstanceRunning(id)) {
        playBtn.innerText = 'Detener';
        playBtn.classList.add('btn-kill');
        playBtn.dataset.action = 'kill';
        playBtn.disabled = false;
    } else {
        playBtn.innerText = 'Jugar';
        playBtn.classList.remove('btn-kill');
        playBtn.dataset.action = 'play';
        playBtn.disabled = false;
    }
}