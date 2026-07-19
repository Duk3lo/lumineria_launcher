import { fetchProfiles, loadSession, syncInstalledProfilesFromDatabase } from './state.js';
import { drawProfiles, updateStatus, initSettingsPanel, initInstanceEventListeners } from './ui.js';
import { iniciarJuego, abrirCarpetaInstancia } from './launcher.js';
import { openLoginModal, closeLoginModal, handleOfflineLogin, handleMicrosoftLogin, restoreSession } from './auth.js';
import { initInstanceDetail, openInstanceDetail } from './instanceDetail.js';
import { initCreator } from './creator.js';
import { loadExploreModpacks, initExplore } from './explore.js';
import { initDialogs } from './dialogs.js';



async function checkForUpdates() {
    try {
        const { check } = window.__TAURI__.updater;
        const { relaunch } = window.__TAURI__.process;
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