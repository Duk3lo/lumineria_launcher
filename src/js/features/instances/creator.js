import { saveProfileToDisk } from '../../core/state.js';
import { invoke } from '../../core/tauri.js';
import { updateStatus, drawProfiles } from '../../ui/ui.js';
import { showAlert } from '../../ui/dialogs.js';

const modal = document.getElementById('new-instance-modal');
const typeSelect = document.getElementById('new-instance-type');
const versionSelect = document.getElementById('new-instance-version');
const nameInput = document.getElementById('new-instance-name');

const loaderPickerGroup = document.getElementById('new-instance-loader-version-group');
const loaderPicker = document.getElementById('loader-picker');
const loaderPickerTrigger = document.getElementById('loader-picker-trigger');
const loaderPickerTriggerText = document.getElementById('loader-picker-trigger-text');
const loaderPickerMenu = document.getElementById('loader-picker-menu');

let mojangVersionsCache = [];
let neoforgeVersionsCache = null;
let forgeVersionsCache = null;
let selectedLoaderVersion = null;

const LOADER_DISPLAY_NAMES = {
    vanilla: 'Vanilla',
    fabric: 'Fabric',
    forge: 'Forge',
    neoforge: 'NeoForge'
};

export function initCreator() {
    document.getElementById('btn-new-instance').addEventListener('click', openCreatorModal);
    document.getElementById('new-instance-close').addEventListener('click', () => modal.classList.add('hidden'));

    typeSelect.addEventListener('change', onTypeOrVersionListChange);
    versionSelect.addEventListener('change', populateLoaderVersions);

    loaderPickerTrigger.addEventListener('click', () => {
        loaderPicker.classList.contains('open') ? closeLoaderPicker() : openLoaderPicker();
    });
    document.addEventListener('click', (e) => {
        if (!loaderPicker.contains(e.target)) closeLoaderPicker();
    });

    document.getElementById('btn-create-instance').addEventListener('click', createInstance);
}

async function openCreatorModal() {
    nameInput.value = '';
    modal.classList.remove('hidden');

    if (mojangVersionsCache.length === 0) {
        versionSelect.innerHTML = '<option>Cargando de Mojang...</option>';
        try {
            const res = await fetch("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json");
            const data = await res.json();
            mojangVersionsCache = data.versions.filter(v => v.type === "release").map(v => v.id);
        } catch (e) {
            versionSelect.innerHTML = '<option>Error al cargar versiones</option>';
            return;
        }
    }
    await onTypeOrVersionListChange();
}

// "1.21.10" -> "21.10" | "1.20.2" -> "20.2" | "1.21" -> "21.0"
function mcVersionToNeoforgePrefix(mcVersion) {
    const parts = mcVersion.split('.');
    if (parts.length >= 3) return `${parts[1]}.${parts[2]}`;
    return `${parts[1]}.0`;
}

async function ensureLoaderCacheForType(type) {
    if (type === 'forge' && !forgeVersionsCache) {
        forgeVersionsCache = await invoke('fetch_forge_versions');
    }
    if (type === 'neoforge' && !neoforgeVersionsCache) {
        neoforgeVersionsCache = await invoke('fetch_neoforge_versions');
    }
}

function isMcVersionSupported(type, mcVersion) {
    if (type === 'vanilla' || type === 'fabric') return true;

    if (type === 'forge') {
        const minor = parseInt(mcVersion.split('.')[1]);
        return minor >= 13 && !!(forgeVersionsCache?.[mcVersion]?.length);
    }
    if (type === 'neoforge') {
        if (!neoforgeVersionsCache) return true;
        const prefix = mcVersionToNeoforgePrefix(mcVersion);
        return neoforgeVersionsCache.some(v => v.startsWith(`${prefix}.`));
    }
    return true;
}

function renderMcVersionOptions(type) {
    const currentValue = versionSelect.value;

    versionSelect.innerHTML = mojangVersionsCache.map(v => {
        const supported = isMcVersionSupported(type, v);
        return `<option value="${v}" ${supported ? '' : 'disabled'}>${v}${supported ? '' : ' — no compatible'}</option>`;
    }).join('');

    if (isMcVersionSupported(type, currentValue)) {
        versionSelect.value = currentValue;
    } else {
        const firstSupported = mojangVersionsCache.find(v => isMcVersionSupported(type, v));
        if (firstSupported) versionSelect.value = firstSupported;
    }
}

async function onTypeOrVersionListChange() {
    const type = typeSelect.value;

    if (type === 'forge' || type === 'neoforge') {
        versionSelect.disabled = true;
        try {
            await ensureLoaderCacheForType(type);
        } catch (e) {
            // si falla la carga, no bloqueamos nada — se deja todo seleccionable
        }
        versionSelect.disabled = false;
    }

    renderMcVersionOptions(type);
    await populateLoaderVersions();
}

// ---- Picker de versión del cargador ----

function openLoaderPicker() {
    if (loaderPickerTrigger.disabled) return;
    loaderPicker.classList.add('open');
    loaderPickerMenu.classList.remove('hidden');
}
function closeLoaderPicker() {
    loaderPicker.classList.remove('open');
    loaderPickerMenu.classList.add('hidden');
}

function setPickerLoading(message) {
    loaderPickerTrigger.disabled = true;
    loaderPickerTriggerText.textContent = message;
    loaderPickerMenu.innerHTML = `<div class="loader-picker-loading">${message}</div>`;
    selectedLoaderVersion = null;
}

function setPickerEmpty(message) {
    loaderPickerTrigger.disabled = true;
    loaderPickerTriggerText.textContent = message;
    loaderPickerMenu.innerHTML = `<div class="loader-picker-empty">${message}</div>`;
    selectedLoaderVersion = null;
}

function setPickerItems(items) {
    if (items.length === 0) {
        setPickerEmpty('Sin versiones disponibles');
        return;
    }

    loaderPickerTrigger.disabled = false;
    loaderPickerMenu.innerHTML = items.map(item => `
        <div class="loader-picker-item" data-value="${item.value}">
            <span>${item.label}</span>
            ${item.badge ? `<span class="loader-picker-badge ${item.badge}">${item.badge === 'latest' ? 'Más reciente' : 'Beta'}</span>` : ''}
        </div>
    `).join('');

    loaderPickerMenu.querySelectorAll('.loader-picker-item').forEach(el => {
        el.addEventListener('click', () => {
            const item = items.find(i => i.value === el.dataset.value);
            selectLoaderVersion(item.value, item.label);
            closeLoaderPicker();
        });
    });

    selectLoaderVersion(items[0].value, items[0].label);
}

function selectLoaderVersion(value, label) {
    selectedLoaderVersion = value;
    loaderPickerTriggerText.textContent = label;
    loaderPickerMenu.querySelectorAll('.loader-picker-item').forEach(el => {
        el.classList.toggle('selected', el.dataset.value === value);
    });
}

async function populateLoaderVersions() {
    const type = typeSelect.value;
    const mcVersion = versionSelect.value;

    if (type === 'vanilla') {
        selectedLoaderVersion = 'latest';
        loaderPickerGroup.classList.add('hidden');
        return;
    }
    loaderPickerGroup.classList.remove('hidden');

    if (type === 'fabric') {
        setPickerLoading('Buscando Fabric...');
        try {
            const res = await fetch(`https://meta.fabricmc.net/v2/versions/loader/${mcVersion}`);
            const loaders = await res.json();
            const items = loaders.map((l, idx) => ({
                value: l.loader.version,
                label: l.loader.version,
                badge: idx === 0 ? 'latest' : (l.loader.stable === false ? 'beta' : null)
            }));
            setPickerItems(items);
        } catch (e) {
            setPickerItems([{ value: '0.15.11', label: '0.15.11 (Default)', badge: null }]);
        }

    } else if (type === 'neoforge') {
        setPickerLoading('Buscando NeoForge...');
        try {
            await ensureLoaderCacheForType('neoforge');
            const prefix = mcVersionToNeoforgePrefix(mcVersion);
            const matches = neoforgeVersionsCache.filter(v => v.startsWith(`${prefix}.`)).reverse();
            const items = matches.map((v, idx) => ({
                value: v,
                label: v,
                badge: idx === 0 ? 'latest' : (v.includes('-beta') ? 'beta' : null)
            }));
            setPickerItems(items);
        } catch (e) {
            setPickerEmpty('Error al buscar NeoForge');
        }

    } else if (type === 'forge') {
        setPickerLoading('Buscando Forge...');
        try {
            await ensureLoaderCacheForType('forge');
            const fullVersions = (forgeVersionsCache[mcVersion] || []).slice().reverse();
            const items = fullVersions.map((full, idx) => {
                const label = full.startsWith(`${mcVersion}-`) ? full.slice(mcVersion.length + 1) : full;
                return { value: full, label, badge: idx === 0 ? 'latest' : null };
            });
            setPickerItems(items);
        } catch (e) {
            setPickerEmpty('Error al buscar Forge');
        }
    }
}


async function createInstance() {
    const name = nameInput.value.trim();
    if (!name) { await showAlert("Escribe un nombre."); return; }

    const mcVersion = versionSelect.value;
    const type = typeSelect.value;

    if (type !== 'vanilla' && !selectedLoaderVersion) {
        await showAlert("No hay una versión de cargador disponible para esta combinación.");
        return;
    }

    const btn = document.getElementById('btn-create-instance');
    btn.innerText = "Creando...";
    btn.disabled = true;

    try {
        const id = "custom-" + name.toLowerCase().replace(/[^a-z0-9]/g, '-') + '-' + Date.now();

        let newProfile = {
            title: name,
            mc_version: mcVersion,
            loader_name: LOADER_DISPLAY_NAMES[type] || type,
            image: 'assets/logo.png',
        };

        const minorVersion = parseInt(mcVersion.split('.')[1]);
        if (type !== 'vanilla') {
            newProfile.java_version = minorVersion >= 20 ? 21 : minorVersion >= 17 ? 17 : 8;
            newProfile.loader_version = selectedLoaderVersion;
        }

        if (type === 'fabric') {
            newProfile.version_id = `fabric-loader-${selectedLoaderVersion}-${mcVersion}`;

        } else if (type === 'neoforge') {
            newProfile.version_id = `neoforge-${selectedLoaderVersion}`;
            newProfile.loader_url = `https://maven.neoforged.net/releases/net/neoforged/neoforge/${selectedLoaderVersion}/neoforge-${selectedLoaderVersion}-installer.jar`;

        } else if (type === 'forge') {
            const forgeOnly = selectedLoaderVersion.startsWith(`${mcVersion}-`)
                ? selectedLoaderVersion.slice(mcVersion.length + 1)
                : selectedLoaderVersion;
            newProfile.version_id = `${mcVersion}-forge-${forgeOnly}`;
            newProfile.loader_url = `https://maven.minecraftforge.net/net/minecraftforge/forge/${selectedLoaderVersion}/forge-${selectedLoaderVersion}-installer.jar`;

        } else {
            newProfile.version_id = mcVersion;
        }

        await saveProfileToDisk(id, newProfile);
        updateStatus(`¡Instancia ${name} creada exitosamente!`);
        modal.classList.add('hidden');
        await drawProfiles();

    } catch (e) {
        await showAlert("Error creando instancia: " + e.message);
    } finally {
        btn.innerText = "Crear Instancia";
        btn.disabled = false;
    }
}
