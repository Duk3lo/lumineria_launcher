import { fetchProfiles, loadSession, syncInstalledProfilesFromDatabase } from './state.js';
import { updater, tauriProcess } from './tauri.js';
import { drawProfiles, updateStatus, initSettingsPanel, initInstanceEventListeners } from '../ui/ui.js';
import { initDialogs } from '../ui/dialogs.js';
import { initConsole } from '../ui/console.js';
import { iniciarJuego, abrirCarpetaInstancia } from '../features/instances/launcher.js';
import { initInstanceDetail, openInstanceDetail } from '../features/instances/instanceDetail.js';
import { initCreator } from '../features/instances/creator.js';
import { openLoginModal, closeLoginModal, handleOfflineLogin, handleMicrosoftLogin, restoreSession } from '../features/auth/auth.js';
import { loadExploreModpacks, initExplore } from '../features/explore/explore.js';

async function checkForUpdates() {
    try {
        const { check } = updater;
        const { relaunch } = tauriProcess;
        const update = await check();
        if (update) {
            updateStatus(`Actualización ${update.version} disponible...`);
            await update.downloadAndInstall();
            await relaunch();
        }
    } catch (e) {
        console.warn('No se pudo comprobar actualizaciones:', e);
    }
}

document.addEventListener('DOMContentLoaded', async () => {
    try {
        updateStatus("Cargando instancias locales...");
        initInstanceDetail();
        initCreator();
        initExplore();
        initInstanceEventListeners();
        await initSettingsPanel();
        await fetchProfiles();
        await syncInstalledProfilesFromDatabase();
        drawProfiles();
        initDialogs();
        initConsole();

        const viewGrid = document.getElementById('view-grid');
        const viewExplore = document.getElementById('view-explore');
        const viewInstance = document.getElementById('view-instance');
        document.getElementById('btn-my-instances').addEventListener('click', (e) => {
            document.querySelectorAll('.game-list li').forEach(li => li.classList.remove('active'));
            e.currentTarget.classList.add('active');

            viewExplore.classList.add('hidden');
            viewInstance.classList.add('hidden');
            viewGrid.classList.remove('hidden');
            drawProfiles();
        });
        document.getElementById('btn-explore-modpacks').addEventListener('click', (e) => {
            document.querySelectorAll('.game-list li').forEach(li => li.classList.remove('active'));
            e.currentTarget.classList.add('active');

            viewGrid.classList.add('hidden');
            viewInstance.classList.add('hidden');
            viewExplore.classList.remove('hidden');
            loadExploreModpacks();
        });
        const savedSession = await loadSession();
        if (savedSession) restoreSession(savedSession);
        document.addEventListener('lumineria:play-profile', (e) => iniciarJuego(e.detail.id, e.detail.force, e.detail.isLocal, e.detail.localProfile));
        document.addEventListener('lumineria:open-folder', (e) => abrirCarpetaInstancia(e.detail.id));
        document.addEventListener('lumineria:open-instance-detail', (e) => openInstanceDetail(e.detail.id, e.detail.isLocal, e.detail.localProfile));
        document.addEventListener('lumineria:open-mods', (e) => {
            openInstanceDetail(e.detail.id);
            document.querySelector('.tab-btn[data-tab="tab-mods"]')?.click();
        });
        document.getElementById('login-btn')?.addEventListener('click', openLoginModal);
        document.getElementById('login-modal-close')?.addEventListener('click', closeLoginModal);
        document.getElementById('login-offline-btn')?.addEventListener('click', () => {
            const username = document.getElementById('login-username-input')?.value || '';
            handleOfflineLogin(username);
        });
        document.getElementById('login-microsoft-btn')?.addEventListener('click', handleMicrosoftLogin);

        checkForUpdates();

    } catch (error) {
        updateStatus("Error al cargar el launcher");
        console.error("Error en main.js:", error);
    }
});
