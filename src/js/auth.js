import { loginOffline, loginMicrosoftStart, loginMicrosoftPoll, saveSession } from './state.js';
import { updateStatus } from './ui.js';

const loginModal = document.getElementById('login-modal');
const loginStatus = document.getElementById('login-status');
const accountLabel = document.getElementById('account-label');
const usernameInput = document.getElementById('login-username-input');

// El login con Microsoft todavía no está disponible (falta terminar la
// configuración de la app en Azure). Por ahora se muestra como "próximamente"
// y no intenta hacer el flujo real.
const MICROSOFT_LOGIN_ENABLED = false;

const USERNAME_REGEX = /^[A-Za-z0-9_]{3,16}$/;

export function openLoginModal() {
    loginModal?.classList.remove('hidden');
}

export function closeLoginModal() {
    loginModal?.classList.add('hidden');
}

function setLoginMessage(message, isError = false) {
    if (!loginStatus) return;
    loginStatus.innerText = message;
    loginStatus.style.color = isError ? 'var(--danger)' : '';
}

export async function handleOfflineLogin(username) {
    const trimmed = (username || '').trim();

    if (!trimmed) {
        setLoginMessage('Ingresá un nombre de usuario para poder jugar.', true);
        usernameInput?.focus();
        return;
    }
    if (!USERNAME_REGEX.test(trimmed)) {
        setLoginMessage('Nombre inválido: usá entre 3 y 16 caracteres (letras, números y "_").', true);
        usernameInput?.focus();
        return;
    }

    try {
        const session = await loginOffline(trimmed);
        await finishLogin(session, "no premium");
    } catch (e) {
        setLoginMessage(`Error: ${e}`, true);
    }
}

export async function handleMicrosoftLogin() {
    if (!MICROSOFT_LOGIN_ENABLED) {
        setLoginMessage('El inicio de sesión con Microsoft estará disponible próximamente.');
        return;
    }

    try {
        const info = await loginMicrosoftStart();
        setLoginMessage(`Andá a ${info.verificationUri} e ingresá el código: ${info.userCode}`);

        const session = await loginMicrosoftPoll(info.deviceCode, info.interval, info.expiresIn);
        await finishLogin(session, "premium");
    } catch (e) {
        setLoginMessage(`Error: ${e}`, true);
    }
}

/** Aplica a la UI una sesión que se recuperó del disco al iniciar el launcher. */
export function restoreSession(session) {
    if (!session) return false;
    updateStatus(`Sesión iniciada como ${session.username}`);
    if (accountLabel) accountLabel.innerText = session.username;
    return true;
}

async function finishLogin(session, tipo) {
    updateStatus(`Sesión iniciada como ${session.username} (${tipo})`);
    if (accountLabel) accountLabel.innerText = session.username;
    setLoginMessage('');
    closeLoginModal();
    await saveSession();
}

document.addEventListener('lumineria:require-login', openLoginModal);