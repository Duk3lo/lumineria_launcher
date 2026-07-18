import { PROFILES, saveProfileToDisk, getBaseDirectory } from './state.js';
import { drawProfiles, updateStatus } from './ui.js';

const { invoke } = window.__TAURI__.core;
import { showAlert } from './dialogs.js';

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
    exploreGrid.innerHTML = '<p class="mods-empty-state">Conectando al servidor oficial...</p>';

    try {
        const baseDir = await getBaseDirectory();
        const databaseModpacks = await invoke('fetch_official_modpacks', { baseDir });

        if (!databaseModpacks || Object.keys(databaseModpacks).length === 0) {
            exploreGrid.innerHTML = '<p class="mods-empty-state">No hay modpacks disponibles actualmente.</p>';
            return;
        }

        exploreGrid.innerHTML = '';

        Object.keys(databaseModpacks).forEach(db_id => {
            const pack = databaseModpacks[db_id];
            const isInstalled = PROFILES[db_id] !== undefined;
            const imageUrl = pack.image || 'assets/logo.png';

            const card = document.createElement('div');
            card.className = 'profile-card';
            card.innerHTML = `
                <div class="profile-card-bg" style="background-image:url('${imageUrl}')"></div>
                <div class="profile-content">
                    <h3 class="profile-title">${pack.title}</h3>
                    <div class="profile-badges">
                        <span class="badge loader">${pack.loader_name}</span>
                        <span class="badge version">${pack.mc_version}</span>
                    </div>
                    <div class="profile-actions" style="margin-top: auto; padding-top: 15px;">
                        <button class="primary-btn btn-install-modpack" style="width: 100%; border-radius: 8px; padding: 10px;" ${isInstalled ? 'disabled' : ''}>
                            ${isInstalled ? '✓ Instalado' : '⬇ Instalar Cliente'}
                        </button>
                    </div>
                </div>
            `;

            const installBtn = card.querySelector('.btn-install-modpack');
            if (!isInstalled) {
                installBtn.addEventListener('click', async () => {
                    installBtn.innerText = "Instalando...";
                    installBtn.disabled = true;
                    await saveProfileToDisk(db_id, pack);
                    updateStatus(`¡${pack.title} añadido correctamente!`);
                    document.getElementById('btn-my-instances').click();
                });
            }

            exploreGrid.appendChild(card);
        });

    } catch (error) {
        console.error("Error en explore.js:", error);
        exploreGrid.innerHTML = `<p class="mods-empty-state" style="color: var(--danger)">Error: No se pudo conectar al servidor.</p>`;
    }
}