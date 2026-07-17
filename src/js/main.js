import { fetchProfiles, loadSession } from './state.js';
import { drawProfiles, updateStatus, initSettingsPanel } from './ui.js';
import { iniciarJuego, abrirCarpetaInstancia } from './launcher.js';
import { openLoginModal, closeLoginModal, handleOfflineLogin, handleMicrosoftLogin, restoreSession } from './auth.js';
import { initInstanceDetail, openInstanceDetail } from './instanceDetail.js';
import { initCreator } from './creator.js'; // <--- ESTO FALTABA Y ROMPÍA TODO

async function checkForUpdates() {
    try {
        const { check } = window.__TAURI__.updater;
        const { relaunch } = window.__TAURI__.process;

        const update = await check();
        if (update) {
            updateStatus(`Actualización ${update.version} disponible, instalando...`);
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

        await fetchProfiles();
        drawProfiles();
        await initSettingsPanel();
        initInstanceDetail();
        initCreator(); // Ahora sí funcionará porque ya lo importamos

        const viewGrid = document.getElementById('view-grid');
        const viewExplore = document.getElementById('view-explore');
        const viewInstance = document.getElementById('view-instance');

        document.getElementById('btn-my-instances').addEventListener('click', (e) => {
            document.querySelectorAll('.game-list li').forEach(li => li.classList.remove('active'));
            e.target.classList.add('active');
            viewExplore.classList.add('hidden');
            viewInstance.classList.add('hidden');
            viewGrid.classList.remove('hidden');
        });

        document.getElementById('btn-explore-modpacks').addEventListener('click', (e) => {
            document.querySelectorAll('.game-list li').forEach(li => li.classList.remove('active'));
            e.target.classList.add('active');
            viewGrid.classList.add('hidden');
            viewInstance.classList.add('hidden');
            viewExplore.classList.remove('hidden');
        });

        const savedSession = await loadSession();
        if (savedSession) {
            restoreSession(savedSession);
        } else {
            updateStatus("Esperando acción...");
        }

        document.addEventListener('lumineria:play-profile', (event) => {
            iniciarJuego(event.detail.id, event.detail.force === true);
        });
        document.addEventListener('lumineria:open-folder', (event) => {
            abrirCarpetaInstancia(event.detail.id);
        });

        document.addEventListener('lumineria:open-mods', (event) => {
            openInstanceDetail(event.detail.id);
            const tabModsBtn = document.querySelector('.tab-btn[data-tab="tab-mods"]');
            if (tabModsBtn) tabModsBtn.click();
        });

        document.addEventListener('lumineria:open-instance-detail', (event) => {
            openInstanceDetail(event.detail.id);
        });

        document.getElementById('login-btn')?.addEventListener('click', openLoginModal);
        document.getElementById('login-modal-close')?.addEventListener('click', closeLoginModal);
        document.getElementById('login-offline-btn')?.addEventListener('click', () => {
            const username = document.getElementById('login-username-input')?.value || '';
            handleOfflineLogin(username);
        });
        document.getElementById('login-microsoft-btn')?.addEventListener('click', handleMicrosoftLogin);

        document.getElementById('btn-minecraft').addEventListener('click', () => console.log('Minecraft seleccionado'));
        document.getElementById('btn-hytale').addEventListener('click', () => alert('Hytale llegará pronto!'));

        checkForUpdates();

    } catch (error) {
        updateStatus("Error al cargar profiles.json");
        console.error(error);
    }
});