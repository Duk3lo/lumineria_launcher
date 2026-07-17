import { PROFILES, saveProfileToDisk } from './state.js';
import { updateStatus, drawProfiles } from './ui.js';

const modal = document.getElementById('new-instance-modal');
const typeSelect = document.getElementById('new-instance-type');
const versionSelect = document.getElementById('new-instance-version');
const nameInput = document.getElementById('new-instance-name');

let mojangVersionsCache = [];

export function initCreator() {
    document.getElementById('btn-new-instance').addEventListener('click', openCreatorModal);
    document.getElementById('new-instance-close').addEventListener('click', () => modal.classList.add('hidden'));
    
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
            // Filtrar solo las releases oficiales
            mojangVersionsCache = data.versions.filter(v => v.type === "release").map(v => v.id);
            populateVersions();
        } catch (e) {
            versionSelect.innerHTML = '<option>Error al cargar versiones</option>';
        }
    }
}

function populateVersions() {
    versionSelect.innerHTML = '';
    mojangVersionsCache.forEach(v => {
        const opt = document.createElement('option');
        opt.value = v;
        opt.innerText = v;
        versionSelect.appendChild(opt);
    });
}

async function createInstance() {
    const name = nameInput.value.trim();
    if(!name) { alert("Escribe un nombre."); return; }
    
    const mcVersion = versionSelect.value;
    const type = typeSelect.value; // "vanilla", "fabric"
    const btn = document.getElementById('btn-create-instance');
    
    btn.innerText = "Creando...";
    btn.disabled = true;

    try {
        const id = name.toLowerCase().replace(/[^a-z0-9]/g, '-') + '-' + Date.now();
        
        let newProfile = {
            title: name,
            mc_version: mcVersion,
            loader_name: type === 'vanilla' ? 'Vanilla' : type === 'fabric' ? 'Fabric' : 'Forge',
            image: 'assets/logo.png',
        };

        // Regla básica de Java (1.17+ usa java 17 o 21, menores usan java 8)
        const minorVersion = parseInt(mcVersion.split('.')[1]);
        if (type !== 'vanilla') {
            newProfile.java_version = minorVersion >= 20 ? 21 : minorVersion >= 17 ? 17 : 8;
        }

        // Lógica de Fabric usando la API Oficial
        if (type === 'fabric') {
            const loaderRes = await fetch(`https://meta.fabricmc.net/v2/versions/loader/${mcVersion}`);
            const loaders = await loaderRes.json();
            if (loaders.length === 0) throw new Error("No hay Fabric para esta versión");
            
            const loaderVersion = loaders[0].loader.version; // El más reciente
            
            newProfile.version_id = `fabric-loader-${loaderVersion}-${mcVersion}`;
            newProfile.loader_url = `https://meta.fabricmc.net/v2/versions/loader/${mcVersion}/${loaderVersion}/1.0.1/server/jar`; 
        } else {
            newProfile.version_id = mcVersion;
        }

        // GUARDAMOS EN DISCO GRACIAS A NUESTRA NUEVA FUNCIÓN
        await saveProfileToDisk(id, newProfile);
        
        updateStatus(`¡Instancia ${name} creada exitosamente!`);
        modal.classList.add('hidden');
        drawProfiles(); 
        
    } catch (e) {
        alert("Error creando instancia: " + e.message);
    } finally {
        btn.innerText = "Crear Instancia";
        btn.disabled = false;
    }
}