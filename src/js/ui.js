import {
    PROFILES,
    setProfileSelection,
    SETTINGS,
    loadSettings,
    saveSettings,
    getSystemRamMb,
    getInstanceStatus,
} from './state.js';

const statusText = document.getElementById('status-text');
const profilesGrid = document.getElementById('profiles-grid');

export function updateStatus(text) {
    statusText.innerText = text;
}

export function drawProfiles() {
    profilesGrid.innerHTML = '';
    const profileKeys = Object.keys(PROFILES);

    if (profileKeys.length === 0) {
        profilesGrid.innerHTML = `<p class="mods-empty-state">No se encontraron instancias en profiles.json</p>`;
        return;
    }

    profileKeys.forEach(id => {
        profilesGrid.appendChild(buildProfileCard(id, PROFILES[id]));
    });

    setProfileSelection(profileKeys[0]);
    highlightSelectedCard(profileKeys[0]);

    refreshAllCardStatuses(profileKeys);
}

function buildProfileCard(id, profile) {
    const card = document.createElement('div');
    card.className = 'profile-card';
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
                <span class="badge loader">${profile.loader_name}</span>
                <span class="badge version">${profile.mc_version}</span>
            </div>

            <div class="card-progress hidden" id="card-progress-${id}">
                <div class="card-progress-bar"><div class="card-progress-fill" id="card-progress-fill-${id}"></div></div>
                <span class="card-progress-label" id="card-progress-label-${id}">Preparando...</span>
            </div>

            <div class="profile-actions">
                <div class="play-button-group">
                    <button class="play-btn-card" id="play-btn-${id}" data-action="play">Jugar</button>
                    <button class="play-dropdown-toggle" id="dropdown-toggle-${id}" data-action="toggle-menu" title="Más opciones">⋮</button>
                    <div class="card-dropdown-menu hidden" id="dropdown-menu-${id}">
                        <button data-action="open-folder">📂 Carpeta de instalación</button>
                        <button data-action="view-mods">🧩 Ver mods (<span id="mods-count-${id}">0</span>)</button>
                        <button data-action="reinstall">🔄 Reinstalar / verificar</button>
                    </div>
                </div>
            </div>
        </div>
    `;

    card.addEventListener('click', (event) => {
        if (event.target.closest('button') && event.target.dataset.action !== 'play') return;
        const actionBtn = event.target.closest('[data-action]');
        if (actionBtn && actionBtn.dataset.action === 'toggle-menu') {
            event.stopPropagation();
            toggleCardDropdown(id);
            return;
        }
        closeAllDropdowns();
        document.dispatchEvent(new CustomEvent('lumineria:open-instance-detail', { detail: { id } }));
    });

    card.addEventListener('click', (event) => {
        if (event.target.closest('button')) return;
        setProfileSelection(id);
        highlightSelectedCard(id);
    });

    card.addEventListener('click', (event) => {
        const actionBtn = event.target.closest('[data-action]');
        if (!actionBtn) return;
        const action = actionBtn.dataset.action;

        if (action === 'toggle-menu') {
            event.stopPropagation();
            toggleCardDropdown(id);
            return;
        }
        if (action === 'play') {
            setProfileSelection(id);
            highlightSelectedCard(id);
            closeAllDropdowns();
            document.dispatchEvent(new CustomEvent('lumineria:play-profile', { detail: { id } }));
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
            setProfileSelection(id);
            document.dispatchEvent(new CustomEvent('lumineria:play-profile', { detail: { id, force: true } }));
            return;
        }
    });

    return card;
}

function highlightSelectedCard(id) {
    document.querySelectorAll('.profile-card').forEach(card => card.classList.remove('selected'));
    const selectedCard = document.getElementById(`card-${id}`);
    if (selectedCard) selectedCard.classList.add('selected');
}

function toggleCardDropdown(id) {
    const menu = document.getElementById(`dropdown-menu-${id}`);
    const isOpen = !menu.classList.contains('hidden');
    closeAllDropdowns();
    if (!isOpen) menu.classList.remove('hidden');
}

export function closeAllDropdowns() {
    document.querySelectorAll('.card-dropdown-menu').forEach(menu => menu.classList.add('hidden'));
}

document.addEventListener('click', (event) => {
    if (!event.target.closest('.play-button-group')) {
        closeAllDropdowns();
    }
});


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
        const openFolderBtn = document.querySelector(`#dropdown-menu-${id} [data-action="open-folder"]`);
        const modsCountLabel = document.getElementById(`mods-count-${id}`);

        if (dot) {
            dot.classList.toggle('installed', status.installed);
            dot.title = status.installed ? 'Instalado' : 'No instalado';
        }
        if (playBtn) {
            playBtn.innerText = status.installed ? 'Jugar' : 'Instalar y jugar';
        }
        if (openFolderBtn) {
            openFolderBtn.disabled = !status.installed;
        }
        if (modsCountLabel) {
            modsCountLabel.innerText = status.modsCount ?? 0;
        }
    } catch (e) {
        console.warn(`No se pudo comprobar el estado de ${id}:`, e);
    }
}

export function setCardPlayState(id, disabled) {
    const playBtn = document.getElementById(`play-btn-${id}`);
    const dropdownToggle = document.getElementById(`dropdown-toggle-${id}`);
    if (playBtn) playBtn.disabled = disabled;
    if (dropdownToggle) dropdownToggle.disabled = disabled;
}

export function updateCardProgress(id, percent, label) {
    const container = document.getElementById(`card-progress-${id}`);
    const fill = document.getElementById(`card-progress-fill-${id}`);
    const labelEl = document.getElementById(`card-progress-label-${id}`);
    if (!container || !fill) return;

    if (percent > 0 && percent < 100) {
        container.classList.remove('hidden');
    }
    fill.style.width = `${percent}%`;
    if (labelEl && label) labelEl.innerText = label;

    if (percent >= 100) {
        setTimeout(() => container.classList.add('hidden'), 1500);
    }
}

export async function initSettingsPanel() {
    await loadSettings();

    const ramMinInput = document.getElementById('ram-min-input');
    const ramMaxInput = document.getElementById('ram-max-input');
    const javaArgsInput = document.getElementById('java-args-input');
    const ramMaxHint = document.getElementById('ram-max-hint');
    const saveBtn = document.getElementById('settings-save-btn');

    if (!ramMinInput || !ramMaxInput) return;

    ramMinInput.value = SETTINGS.ramMinMb;
    ramMaxInput.value = SETTINGS.ramMaxMb;
    if (javaArgsInput) javaArgsInput.value = SETTINGS.javaArgsExtra || "";

    try {
        const totalRam = await getSystemRamMb();
        ramMaxInput.max = totalRam;
        if (ramMaxHint) ramMaxHint.innerText = `RAM total del sistema: ${totalRam} MB`;
    } catch (e) {
        console.warn('No se pudo detectar la RAM del sistema:', e);
    }

    saveBtn?.addEventListener('click', async () => {
        const ramMinMb = parseInt(ramMinInput.value, 10);
        const ramMaxMb = parseInt(ramMaxInput.value, 10);

        if (isNaN(ramMinMb) || isNaN(ramMaxMb) || ramMinMb <= 0 || ramMaxMb < ramMinMb) {
            updateStatus("Valores de RAM inválidos");
            return;
        }

        await saveSettings({
            ramMinMb,
            ramMaxMb,
            javaArgsExtra: javaArgsInput ? javaArgsInput.value : ""
        });
        updateStatus("Ajustes guardados");
    });
}