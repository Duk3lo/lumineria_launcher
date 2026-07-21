import { getInstanceDir } from '../../core/state.js';
import { invoke } from '../../core/tauri.js';

const rpListEl = document.getElementById('rp-list');
const rpCountLabel = document.getElementById('rp-count-label');
const rpSearchInput = document.getElementById('rp-search-input');

let currentProfileId = null;
let currentRPs = [];

export async function renderResourcePacksForInstance(profileId) {
    currentProfileId = profileId;
    rpListEl.innerHTML = `<p class="mods-empty-state">Cargando paquetes de recursos...</p>`;
    rpSearchInput.value = '';

    try {
        const instanceDir = await getInstanceDir(profileId);
        currentRPs = await invoke('list_resource_packs', { instanceDir });
        renderRPList(currentRPs);
    } catch (e) {
        rpListEl.innerHTML = `<p class="mods-empty-state">Error al leer resourcepacks: ${e}</p>`;
    }
}

function renderRPList(rps) {
    const activeCount = rps.filter(r => r.enabled).length;
    rpCountLabel.innerText = `${activeCount} / ${rps.length} activos`;

    if (rps.length === 0) {
        rpListEl.innerHTML = `<p class="mods-empty-state">No hay paquetes de recursos instalados.</p>`;
        return;
    }

    rpListEl.innerHTML = '';
    rps.forEach(rp => {
        const row = document.createElement('div');
        row.className = `mod-row ${rp.enabled ? '' : 'disabled-row'}`;
        row.innerHTML = `
            <div class="mod-icon" style="background: linear-gradient(135deg, #10b981, #059669);">🎨</div>
            <div class="mod-info">
                <span class="mod-name">${rp.displayName}</span>
                <span class="mod-meta">${rp.sizeKb} KB · ${rp.enabled ? 'Activo' : 'Desactivado'}</span>
            </div>
            <label class="toggle-switch">
                <input type="checkbox" ${rp.enabled ? 'checked' : ''} data-filename="${rp.filename}">
                <span class="toggle-slider"></span>
            </label>
        `;

        const checkbox = row.querySelector('input');
        checkbox.addEventListener('change', () => handleRPToggle(rp, checkbox));
        rpListEl.appendChild(row);
    });
}

async function handleRPToggle(rp, checkboxEl) {
    checkboxEl.disabled = true;
    try {
        const instanceDir = await getInstanceDir(currentProfileId);
        const newFilename = await invoke('toggle_resource_pack', { instanceDir, filename: rp.filename, enable: checkboxEl.checked });
        rp.filename = newFilename;
        rp.enabled = checkboxEl.checked;
        const row = checkboxEl.closest('.mod-row');
        row.classList.toggle('disabled-row', !rp.enabled);
        row.querySelector('.mod-meta').innerText = `${rp.sizeKb} KB · ${rp.enabled ? 'Activo' : 'Desactivado'}`;
        const activeCount = currentRPs.filter(r => r.enabled).length;
        rpCountLabel.innerText = `${activeCount} / ${currentRPs.length} activos`;
    } catch (e) {
        checkboxEl.checked = !checkboxEl.checked;
    } finally {
        checkboxEl.disabled = false;
    }
}

rpSearchInput?.addEventListener('input', () => {
    const query = rpSearchInput.value.trim().toLowerCase();
    const filtered = query ? currentRPs.filter(r => r.displayName.toLowerCase().includes(query)) : currentRPs;
    renderRPList(filtered);
});
