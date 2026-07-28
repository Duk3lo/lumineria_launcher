import { fetchProfiles, syncInstalledProfilesFromDatabase } from './state.js';
import { updater, tauriProcess, invoke } from './tauri.js';
import { drawProfiles, updateStatus, initSettingsPanel, initInstanceEventListeners } from '../ui/ui.js';
import { initDialogs } from '../ui/dialogs.js';
import { initConsole } from '../ui/console.js';
import { iniciarJuego, abrirCarpetaInstancia, requestCancelPreparation } from '../features/instances/launcher.js';
import { initInstanceDetail, openInstanceDetail } from '../features/instances/instanceDetail.js';
import { initCreator } from '../features/instances/creator.js';
import { closeLoginModal, handleOfflineLogin, handleMicrosoftLogin, handleMicrosoftLoginCancel, initAuth, handleAccountButton, closeAccountsModal, handleAddAccount } from '../features/auth/auth.js';
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

function withTimeout(promise, ms) {
    return Promise.race([
        promise,
        new Promise((_, reject) => setTimeout(() => reject(new Error('Tiempo de espera agotado')), ms))
    ]);
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
        drawProfiles();
        initDialogs();
        initConsole();

        withTimeout(syncInstalledProfilesFromDatabase(), 8000)
            .then(() => drawProfiles())
            .catch(e => console.warn('Catálogo remoto no disponible, se continúa con datos locales:', e));

        const viewGrid = document.getElementById('view-grid');
        const viewExplore = document.getElementById('view-explore');
        const viewInstance = document.getElementById('view-instance');
        document.getElementById('login-microsoft-btn')?.addEventListener('click', handleMicrosoftLogin);
        document.getElementById('login-microsoft-cancel-btn')?.addEventListener('click', handleMicrosoftLoginCancel);
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
        await initAuth();
        document.addEventListener('lumineria:play-profile', (e) => iniciarJuego(e.detail.id, e.detail.force, e.detail.isLocal, e.detail.localProfile));
        document.addEventListener('lumineria:open-folder', (e) => abrirCarpetaInstancia(e.detail.id));
        document.addEventListener('lumineria:open-instance-detail', (e) => openInstanceDetail(e.detail.id, e.detail.isLocal, e.detail.localProfile));
        document.addEventListener('lumineria:open-mods', (e) => {
            openInstanceDetail(e.detail.id);
            document.querySelector('.tab-btn[data-tab="tab-mods"]')?.click();
        });
        document.addEventListener('lumineria:cancel-preparation', async (e) => {
            requestCancelPreparation(e.detail.id);
            try {
                await invoke('cancel_preparation', { profileId: e.detail.id });
            } catch (err) {
                console.warn('No se pudo cancelar en el backend:', err);
            }
        });
        document.getElementById('login-btn')?.addEventListener('click', handleAccountButton);
        document.getElementById('accounts-modal-close')?.addEventListener('click', closeAccountsModal);
        document.getElementById('account-add-btn')?.addEventListener('click', handleAddAccount);
        document.getElementById('login-modal-close')?.addEventListener('click', closeLoginModal);
        document.getElementById('login-offline-btn')?.addEventListener('click', () => {
            const username = document.getElementById('login-username-input')?.value || '';
            handleOfflineLogin(username);
        });

        checkForUpdates();

    } catch (error) {
        updateStatus("Error al cargar el launcher");
        console.error("Error en main.js:", error);
    }
});
