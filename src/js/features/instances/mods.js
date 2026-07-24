import { listMods, toggleMod } from '../../core/state.js';
import { refreshCardStatus } from '../../ui/ui.js';

const modsListEl = document.getElementById('mods-list');
const modsCountLabel = document.getElementById('mods-count-label');
const modsSearchInput = document.getElementById('mods-search-input');

let currentProfileId = null;
let currentMods = [];

export async function renderModsForInstance(profileId) {
    currentProfileId = profileId;
    modsListEl.innerHTML = `<p class="mods-empty-state">Cargando mods...</p>`;
    modsSearchInput.value = '';

    try {
        currentMods = await listMods(profileId);
        renderModsList(currentMods);
    } catch (e) {
        modsListEl.innerHTML = `<p class="mods-empty-state">Error al leer la carpeta de mods: ${e}</p>`;
        console.error(e);
    }
}

function renderModsList(mods) {
    const activeCount = mods.filter(m => m.enabled).length;
    modsCountLabel.innerText = `${activeCount} / ${mods.length} activos`;

    if (mods.length === 0) {
        modsListEl.innerHTML = `<p class="mods-empty-state">Esta instancia todavía no tiene mods instalados. Jugala una vez para sincronizarlos.</p>`;
        return;
    }

    modsListEl.innerHTML = '';
    mods.forEach(mod => {
        const row = document.createElement('div');
        row.className = `mod-row ${mod.enabled ? '' : 'disabled-row'}`;
        row.id = `mod-row-${sanitizeId(mod.filename)}`;

        row.innerHTML = `
            <div class="mod-icon">🧩</div>
            <div class="mod-info">
                <span class="mod-name">${mod.displayName}</span>
                <span class="mod-meta">${mod.sizeKb} KB · ${mod.enabled ? 'Activo' : 'Desactivado'}</span>
            </div>
            <label class="toggle-switch">
                <input type="checkbox" ${mod.enabled ? 'checked' : ''} data-filename="${mod.filename}">
                <span class="toggle-slider"></span>
            </label>
        `;

        const checkbox = row.querySelector('input');
        checkbox.addEventListener('change', () => handleToggle(mod, checkbox));
        modsListEl.appendChild(row);
    });
}

async function handleToggle(mod, checkboxEl) {
    if (!currentProfileId) return;
    checkboxEl.disabled = true;

    try {
        const newFilename = await toggleMod(currentProfileId, mod.filename, checkboxEl.checked);
        mod.filename = newFilename;
        mod.enabled = checkboxEl.checked;

        const row = checkboxEl.closest('.mod-row');
        row.classList.toggle('disabled-row', !mod.enabled);
        row.querySelector('.mod-meta').innerText = `${mod.sizeKb} KB · ${mod.enabled ? 'Activo' : 'Desactivado'}`;
        checkboxEl.dataset.filename = newFilename;

        const activeCount = currentMods.filter(m => m.enabled).length;
        modsCountLabel.innerText = `${activeCount} / ${currentMods.length} activos`;

        refreshCardStatus(currentProfileId);
    } catch (e) {
        checkboxEl.checked = !checkboxEl.checked;
        console.error('No se pudo cambiar el estado del mod:', e);
    } finally {
        checkboxEl.disabled = false;
    }
}

function sanitizeId(filename) {
    return filename.replace(/[^a-zA-Z0-9]/g, '-');
}

modsSearchInput?.addEventListener('input', () => {
    const query = modsSearchInput.value.trim().toLowerCase();
    const filtered = query
        ? currentMods.filter(m => m.displayName.toLowerCase().includes(query))
        : currentMods;
    renderModsList(filtered);
});

document.addEventListener('lumineria:mods-updated', (e) => {
    if (e.detail.id === currentProfileId) {
        renderModsForInstance(currentProfileId);
    }
});