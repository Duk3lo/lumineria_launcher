import { fetchProfiles, loadSession } from './state.js';
import { drawProfiles, updateStatus, initSettingsPanel } from './ui.js';
import { iniciarJuego, abrirCarpetaInstancia } from './launcher.js';
import { openLoginModal, closeLoginModal, handleOfflineLogin, handleMicrosoftLogin, restoreSession } from './auth.js';
import { openModsModal, closeModsModal } from './mods.js';
import { initConsole } from './console.js';

// Autoactualización: se resuelve adentro de la función (no en el top-level)
// para que, si el plugin todavía no está listo, no tumbe el arranque de
// todo el launcher.
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
        initConsole();

        // Si ya se había iniciado sesión antes, la recuperamos para no
        // tener que pedirla de nuevo cada vez que se abre el launcher.
        const savedSession = await loadSession();
        if (savedSession) {
            restoreSession(savedSession);
        } else {
            updateStatus("Esperando acción...");
        }

        // Acciones disparadas desde las tarjetas de perfil (ui.js)
        document.addEventListener('lumineria:play-profile', (event) => {
            iniciarJuego(event.detail.id, event.detail.force === true);
        });
        document.addEventListener('lumineria:open-folder', (event) => {
            abrirCarpetaInstancia(event.detail.id);
        });
        document.addEventListener('lumineria:open-mods', (event) => {
            openModsModal(event.detail.id);
        });

        document.getElementById('mods-modal-close')?.addEventListener('click', closeModsModal);

        document.getElementById('login-btn')?.addEventListener('click', openLoginModal);
        document.getElementById('login-modal-close')?.addEventListener('click', closeLoginModal);
        document.getElementById('login-offline-btn')?.addEventListener('click', () => {
            const username = document.getElementById('login-username-input')?.value || '';
            handleOfflineLogin(username);
        });
        document.getElementById('login-microsoft-btn')?.addEventListener('click', handleMicrosoftLogin);

        document.getElementById('btn-minecraft').addEventListener('click', () => console.log('Minecraft seleccionado'));
        document.getElementById('btn-hytale').addEventListener('click', () => alert('Hytale llegará pronto!'));

        // No usamos await: que corra en segundo plano mientras el usuario
        // ya puede usar el launcher con normalidad.
        checkForUpdates();

    } catch (error) {
        updateStatus("Error al cargar profiles.json");
        console.error(error);
    }
});