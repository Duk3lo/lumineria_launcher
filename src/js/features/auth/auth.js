import { loginOffline, loginMicrosoftStart, loginMicrosoftPoll, loginMicrosoftLegacy, cancelMicrosoftLogin, saveSession, clearSession, setAuthSession } from '../../core/state.js';
import { updateStatus } from '../../ui/ui.js';
import { showConfirm } from '../../ui/dialogs.js';

const loginModal = document.getElementById('login-modal');
const loginStatus = document.getElementById('login-status');
const accountLabel = document.getElementById('account-label');
const usernameInput = document.getElementById('login-username-input');
const msLoginBtn = document.getElementById('login-microsoft-btn');
const msCancelBtn = document.getElementById('login-microsoft-cancel-btn');
const msCountdown = document.getElementById('login-ms-countdown');
const logoutBtn = document.getElementById('logout-btn');
const accountSublabel = document.getElementById('account-sublabel');

const MICROSOFT_LOGIN_ENABLED = true;

const USERNAME_REGEX = /^[A-Za-z0-9_]{3,16}$/;

let countdownInterval = null;
let msLoginInProgress = false;

export function openLoginModal() {
    loginModal?.classList.remove('hidden');
}

export function closeLoginModal() {
    if (msLoginInProgress) {
        cancelMicrosoftLogin();
    }
    loginModal?.classList.add('hidden');
}

function setLoginMessage(message, isError = false) {
    if (!loginStatus) return;
    loginStatus.innerText = message;
    loginStatus.style.color = isError ? 'var(--danger)' : '';
}

function startCountdown(expiresIn) {
    let remaining = expiresIn;
    updateCountdownText(remaining);
    countdownInterval = setInterval(() => {
        remaining -= 1;
        if (remaining <= 0) {
            stopCountdown();
            return;
        }
        updateCountdownText(remaining);
    }, 1000);
}

function updateCountdownText(remaining) {
    if (!msCountdown) return;
    const mins = Math.floor(remaining / 60);
    const secs = remaining % 60;
    msCountdown.innerText = `Expira en ${mins}:${secs.toString().padStart(2, '0')}`;
}

function stopCountdown() {
    if (countdownInterval) {
        clearInterval(countdownInterval);
        countdownInterval = null;
    }
    if (msCountdown) msCountdown.innerText = '';
}

function setMsLoginUiState(inProgress) {
    msLoginInProgress = inProgress;
    if (msLoginBtn) msLoginBtn.classList.toggle('hidden', inProgress);
    if (msCancelBtn) msCancelBtn.classList.toggle('hidden', !inProgress);
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
        await finishLogin(session);
    } catch (e) {
        setLoginMessage(`Error: ${e}`, true);
    }
}

export async function handleMicrosoftLogin() {
    if (!MICROSOFT_LOGIN_ENABLED) {
        setLoginMessage('El inicio de sesión con Microsoft estará disponible próximamente.');
        return;
    }
    if (msLoginInProgress) return;

    let info;
    try {
        info = await loginMicrosoftStart();
    } catch (e) {
        // Si falla por el Client ID de Azure, salta a la ventana emergente web (Legacy)
        if (String(e).includes('no reconoce el client_id')) {
            await microsoftLegacyLogin();
            return;
        }
        setLoginMessage(`Error: ${e}`, true);
        return;
    }

    try {
        setLoginMessage(`Andá a ${info.verificationUri} e ingresá el código: ${info.userCode}`);
        setMsLoginUiState(true);
        startCountdown(info.expiresIn);

        const session = await loginMicrosoftPoll(info.deviceCode, info.interval, info.expiresIn);
        stopCountdown();
        setMsLoginUiState(false);
        await finishLogin(session);
    } catch (e) {
        stopCountdown();
        setMsLoginUiState(false);
        setLoginMessage(`Error: ${e}`, true);
    }
}

async function microsoftLegacyLogin() {
    setLoginMessage('Abriendo la ventana de Microsoft...');
    try {
        const session = await loginMicrosoftLegacy();
        await finishLogin(session);
    } catch (e) {
        setLoginMessage(`Error: ${e}`, true);
    }
}

export async function handleMicrosoftLoginCancel() {
    if (!msLoginInProgress) return;
    await cancelMicrosoftLogin();
    setLoginMessage('Inicio de sesión cancelado.');
}

function tipoDeCuenta(session) {
    if (session.userType !== 'msa') return 'no premium';
    return session.ownsMinecraft ? 'premium' : 'sin licencia';
}

function setLoggedInUI(session) {
    if (accountLabel) accountLabel.innerText = session.username;
    if (accountSublabel) accountSublabel.innerText = `Conectado (${tipoDeCuenta(session)})`;
    logoutBtn?.classList.remove('hidden');
}

function setLoggedOutUI() {
    if (accountLabel) accountLabel.innerText = 'Iniciar sesión';
    if (accountSublabel) accountSublabel.innerText = '';
    logoutBtn?.classList.add('hidden');
}

export async function handleLogout() {
    const confirmado = await showConfirm(`¿Cerrar la sesión de "${accountLabel?.innerText}"?`);
    if (!confirmado) return;
    await clearSession();
    setAuthSession(null);
    setLoggedOutUI();
    updateStatus('Sesión cerrada. Iniciá sesión para poder jugar.');
}

export function restoreSession(session) {
    if (!session) return false;
    updateStatus(`Sesión iniciada como ${session.username} (${tipoDeCuenta(session)})`);
    setLoggedInUI(session);
    return true;
}

async function finishLogin(session) {
    updateStatus(`Sesión iniciada como ${session.username} (${tipoDeCuenta(session)})`);
    setLoggedInUI(session);
    setLoginMessage('');
    closeLoginModal();
    await saveSession();
}

document.addEventListener('lumineria:require-login', openLoginModal);