import { PROFILES, saveProfileToDisk, getBaseDirectory } from '../../core/state.js';
import { invoke } from '../../core/tauri.js';
import { drawProfiles, updateStatus } from '../../ui/ui.js';
import { showAlert } from '../../ui/dialogs.js';
import { setSafeBackgroundImage } from '../../core/domSafety.js';

function buildModpackCard(db_id, pack, isInstalled) {
    const imageUrl = pack.image || 'assets/logo.png';

    const card = document.createElement('div');
    card.className = 'profile-card';

    const bg = document.createElement('div');
    bg.className = 'profile-card-bg';
    setSafeBackgroundImage(bg, imageUrl);

    const content = document.createElement('div');
    content.className = 'profile-content';

    const h3 = document.createElement('h3');
    h3.className = 'profile-title';
    h3.textContent = pack.title;

    const badges = document.createElement('div');
    badges.className = 'profile-badges';
    const loaderBadge = document.createElement('span');
    loaderBadge.className = 'badge loader';
    loaderBadge.textContent = pack.loader_name;
    const versionBadge = document.createElement('span');
    versionBadge.className = 'badge version';
    versionBadge.textContent = pack.mc_version;
    badges.append(loaderBadge, versionBadge);

    const actions = document.createElement('div');
    actions.className = 'profile-actions';
    actions.style.marginTop = 'auto';
    actions.style.paddingTop = '15px';

    const installBtn = document.createElement('button');
    installBtn.className = 'primary-btn btn-install-modpack';
    installBtn.style.width = '100%';
    installBtn.style.borderRadius = '8px';
    installBtn.style.padding = '10px';
    installBtn.disabled = isInstalled;
    installBtn.textContent = isInstalled ? '✓ Instalado' : '⬇ Instalar Cliente';
    actions.appendChild(installBtn);

    content.append(h3, badges, actions);
    card.append(bg, content);

    if (!isInstalled) {
        installBtn.addEventListener('click', async () => {
            installBtn.textContent = "Instalando...";
            installBtn.disabled = true;
            await saveProfileToDisk(db_id, pack);
            updateStatus(`¡${pack.title} añadido correctamente!`);
            document.getElementById('btn-my-instances').click();
        });
    }

    return card;
}

export function initExplore() {
    document.getElementById('btn-refresh-explore')?.addEventListener('click', () => {
        loadExploreModpacks();
    });

    const modal = document.getElementById('server-url-modal');
    const input = document.getElementById('server-url-input');

    document.getElementById('btn-change-server')?.addEventListener('click', async () => {
        const baseDir = await getBaseDirectory();
        const config = await invoke('load_launcher_config', { baseDir });
        input.value = config.api_url || '';
        modal.classList.remove('hidden');
    });

    document.getElementById('server-url-close')?.addEventListener('click', () => {
        modal.classList.add('hidden');
    });

    document.getElementById('server-url-save-btn')?.addEventListener('click', async () => {
        const url = input.value.trim();
        if (!url) { await showAlert('Ingresá una URL válida.'); return; }
        const baseDir = await getBaseDirectory();
        await invoke('save_launcher_config', { baseDir, apiUrl: url });
        modal.classList.add('hidden');
        loadExploreModpacks();
    });
}

export async function loadExploreModpacks() {
    const exploreGrid = document.getElementById('explore-grid');
    exploreGrid.textContent = '';

    const loadingMsg = document.createElement('p');
    loadingMsg.className = 'mods-empty-state';
    loadingMsg.textContent = 'Conectando al servidor oficial...';
    exploreGrid.appendChild(loadingMsg);

    try {
        const baseDir = await getBaseDirectory();
        const databaseModpacks = await invoke('fetch_official_modpacks', { baseDir });

        exploreGrid.textContent = '';

        if (!databaseModpacks || Object.keys(databaseModpacks).length === 0) {
            const emptyMsg = document.createElement('p');
            emptyMsg.className = 'mods-empty-state';
            emptyMsg.textContent = 'No hay modpacks disponibles actualmente.';
            exploreGrid.appendChild(emptyMsg);
            return;
        }

        Object.keys(databaseModpacks).forEach(db_id => {
            const pack = databaseModpacks[db_id];
            const isInstalled = PROFILES[db_id] !== undefined;
            exploreGrid.appendChild(buildModpackCard(db_id, pack, isInstalled));
        });

    } catch (error) {
        console.error("Error en explore.js:", error);
        exploreGrid.textContent = '';
        const errMsg = document.createElement('p');
        errMsg.className = 'mods-empty-state';
        errMsg.style.color = 'var(--danger)';
        errMsg.textContent = 'Error: No se pudo conectar al servidor.';
        exploreGrid.appendChild(errMsg);
    }
}