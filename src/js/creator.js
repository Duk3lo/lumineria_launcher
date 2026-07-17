import { PROFILES, saveProfileToDisk } from './state.js';
import { updateStatus, drawProfiles } from './ui.js';

const modal = document.getElementById('new-instance-modal');
const typeSelect = document.getElementById('new-instance-type');
const versionSelect = document.getElementById('new-instance-version');
const loaderVersionSelect = document.getElementById('new-instance-loader-version');
const nameInput = document.getElementById('new-instance-name');

let mojangVersionsCache = [];

export function initCreator() {
    document.getElementById('btn-new-instance').addEventListener('click', openCreatorModal);
    document.getElementById('new-instance-close').addEventListener('click', () => modal.classList.add('hidden'));
    typeSelect.addEventListener('change', populateLoaderVersions);
    versionSelect.addEventListener('change', populateLoaderVersions);

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
            versionSelect.innerHTML = mojangVersionsCache.map(v => `<option value="${v}">${v}</option>`).join('');
            populateLoaderVersions();
        } catch (e) {
            versionSelect.innerHTML = '<option>Error al cargar versiones</option>';
        }
    }
}

async function populateLoaderVersions() {
    const type = typeSelect.value;
    const mcVersion = versionSelect.value;
    
    if (!loaderVersionSelect) return;

    if (type === 'fabric') {
        loaderVersionSelect.innerHTML = '<option>Buscando Fabric...</option>';
        try {
            const res = await fetch(`https://meta.fabricmc.net/v2/versions/loader/${mcVersion}`);
            const loaders = await res.json();
            if (loaders.length > 0) {
                loaderVersionSelect.innerHTML = loaders.map(l => `<option value="${l.loader.version}">${l.loader.version}</option>`).join('');
            } else {
                loaderVersionSelect.innerHTML = '<option value="">No disponible</option>';
            }
        } catch(e) { 
            loaderVersionSelect.innerHTML = '<option value="0.15.11">0.15.11 (Default)</option>'; 
        }
    } else {
        loaderVersionSelect.innerHTML = '<option value="latest">N/A (Vanilla)</option>';
    }
}

async function createInstance() {
    const name = nameInput.value.trim();
    if(!name) { alert("Escribe un nombre."); return; }
    
    const mcVersion = versionSelect.value;
    const type = typeSelect.value;
    const loaderVersion = loaderVersionSelect ? loaderVersionSelect.value : null;
    const btn = document.getElementById('btn-create-instance');
    
    btn.innerText = "Creando...";
    btn.disabled = true;

    try {
        const id = "custom-" + name.toLowerCase().replace(/[^a-z0-9]/g, '-') + '-' + Date.now();
        
        let newProfile = {
            title: name,
            mc_version: mcVersion,
            loader_name: type.charAt(0).toUpperCase() + type.slice(1),
            image: 'assets/logo.png',
        };

        const minorVersion = parseInt(mcVersion.split('.')[1]);
        if (type !== 'vanilla') {
            newProfile.java_version = minorVersion >= 20 ? 21 : minorVersion >= 17 ? 17 : 8;
        }

        if (type === 'fabric' && loaderVersion) {
            newProfile.version_id = `fabric-loader-${loaderVersion}-${mcVersion}`;
            newProfile.loader_url = `https://meta.fabricmc.net/v2/versions/loader/${mcVersion}/${loaderVersion}/1.0.1/server/jar`; 
        } else {
            newProfile.version_id = mcVersion;
        }

        await saveProfileToDisk(id, newProfile);
        updateStatus(`¡Instancia ${name} creada exitosamente!`);
        modal.classList.add('hidden');
        await drawProfiles(); 
        
    } catch (e) {
        alert("Error creando instancia: " + e.message);
    } finally {
        btn.innerText = "Crear Instancia";
        btn.disabled = false;
    }
}