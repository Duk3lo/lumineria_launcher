import {
    ACCOUNTS, ACTIVE_ACCOUNT_ID, AUTH_SESSION, accountId,
    loginOffline, loginMicrosoftStart, loginMicrosoftPoll, loginMicrosoftLegacy, cancelMicrosoftLogin,
    fetchAccounts, restoreAccounts, addAccount, removeAccount, setActiveAccount,
} from '../../core/state.js';
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
const accountAvatar = document.getElementById('account-avatar');
const accountsModal = document.getElementById('accounts-modal');
const accountsList = document.getElementById('accounts-list');

const MICROSOFT_LOGIN_ENABLED = true;
const USERNAME_REGEX = /^[A-Za-z0-9_]{3,16}$/;

let countdownInterval = null;
let msLoginInProgress = false;
let volverASelector = false;

export async function initAuth() {
    try {
        await fetchAccounts();
    } catch (e) {
        console.warn('No se pudieron leer las cuentas:', e);
        updateStatus('Error al leer cuentas guardadas.');
        return;
    }

    try {
        await restoreAccounts();
    } catch (e) {
        console.warn('No se pudo renovar la sesión activa:', e);
    }

    renderAccountUI();
    if (AUTH_SESSION) {
        updateStatus(`Sesión iniciada como ${AUTH_SESSION.username} (${tipoDeCuenta(AUTH_SESSION)})`);
    }
}

function tipoDeCuenta(session) {
    if (session.userType !== 'msa') return 'no premium';
    return session.ownsMinecraft ? 'premium' : 'sin licencia';
}

function avatarDeCuenta(session) {
    if (session.userType !== 'msa') return '👤';
    return session.ownsMinecraft ? '👑' : '⚠️';
}

export function renderAccountUI() {
    if (AUTH_SESSION) {
        if (accountLabel) accountLabel.innerText = AUTH_SESSION.username;
        if (accountSublabel) accountSublabel.innerText = `Conectado (${tipoDeCuenta(AUTH_SESSION)})`;
        if (accountAvatar) accountAvatar.innerText = avatarDeCuenta(AUTH_SESSION);
        logoutBtn?.classList.remove('hidden');
    } else {
        if (accountLabel) accountLabel.innerText = 'Iniciar sesión';
        if (accountSublabel) accountSublabel.innerText = '';
        if (accountAvatar) accountAvatar.innerText = '👤';
        logoutBtn?.classList.add('hidden');
    }
    renderAccountsList();
}

function renderAccountsList() {
    if (!accountsList) return;
    accountsList.replaceChildren();

    if (!ACCOUNTS.length) {
        const vacio = document.createElement('p');
        vacio.className = 'hint-text';
        vacio.innerText = 'Todavía no agregaste ninguna cuenta.';
        accountsList.appendChild(vacio);
        return;
    }

    for (const cuenta of ACCOUNTS) {
        const id = accountId(cuenta);
        const fila = document.createElement('div');
        fila.className = 'account-item';
        fila.classList.toggle('active', id === ACTIVE_ACCOUNT_ID);

        const seleccionar = document.createElement('button');
        seleccionar.className = 'account-item-main';
        seleccionar.addEventListener('click', () => handleSwitchAccount(id));

        const avatar = document.createElement('span');
        avatar.className = 'account-avatar';
        avatar.innerText = avatarDeCuenta(cuenta);

        const textos = document.createElement('span');
        textos.className = 'account-item-text';
        const nombre = document.createElement('span');
        nombre.className = 'account-item-name';
        nombre.innerText = cuenta.username;
        const tipo = document.createElement('span');
        tipo.className = 'account-item-type';
        tipo.innerText = tipoDeCuenta(cuenta);
        textos.append(nombre, tipo);

        seleccionar.append(avatar, textos);

        if (id === ACTIVE_ACCOUNT_ID) {
            const marca = document.createElement('span');
            marca.className = 'account-item-check';
            marca.innerText = '✓';
            seleccionar.appendChild(marca);
        }

        const quitar = document.createElement('button');
        quitar.className = 'account-item-remove';
        quitar.title = `Quitar la cuenta ${cuenta.username}`;
        quitar.innerText = '✕';
        quitar.addEventListener('click', () => handleRemoveAccount(id, cuenta.username));

        fila.append(seleccionar, quitar);
        accountsList.appendChild(fila);
    }
}

export function handleAccountButton() {
    if (ACCOUNTS.length) {
        openAccountsModal();
    } else {
        openLoginModal();
    }
}

export function openAccountsModal() {
    renderAccountsList();
    accountsModal?.classList.remove('hidden');
}

export function closeAccountsModal() {
    accountsModal?.classList.add('hidden');
}

export function handleAddAccount() {
    volverASelector = true;
    closeAccountsModal();
    openLoginModal();
}

async function handleSwitchAccount(id) {
    if (id === ACTIVE_ACCOUNT_ID) {
        closeAccountsModal();
        return;
    }
    try {
        updateStatus('Cambiando de cuenta...');
        await setActiveAccount(id);
    } catch (e) {
        updateStatus(`No se pudo cambiar de cuenta: ${e}`);
        return;
    }

    renderAccountUI();

    if (ACTIVE_ACCOUNT_ID !== id) {
        updateStatus('Esa cuenta venció y se quitó. Agregala de nuevo para usarla.');
        return;
    }

    updateStatus(`Ahora jugás como ${AUTH_SESSION.username} (${tipoDeCuenta(AUTH_SESSION)})`);
    closeAccountsModal();
}

async function handleRemoveAccount(id, username) {
    const confirmado = await showConfirm(`¿Quitar la cuenta "${username}"?`);
    if (!confirmado) return;

    try {
        await removeAccount(id);
    } catch (e) {
        updateStatus(`No se pudo quitar la cuenta: ${e}`);
        return;
    }

    renderAccountUI();
    updateStatus(AUTH_SESSION
        ? `Cuenta quitada. Ahora jugás como ${AUTH_SESSION.username}.`
        : 'Cuenta quitada. Agregá una para poder jugar.');
}

export async function handleLogout() {
    if (!AUTH_SESSION) return;
    await handleRemoveAccount(accountId(AUTH_SESSION), AUTH_SESSION.username);
}

export function openLoginModal() {
    setLoginMessage('');
    loginModal?.classList.remove('hidden');
}

export function closeLoginModal() {
    if (msLoginInProgress) {
        cancelMicrosoftLogin();
        stopCountdown();
        setMsLoginUiState(false);
    }
    loginModal?.classList.add('hidden');
    if (volverASelector) {
        volverASelector = false;
        openAccountsModal();
    }
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
    stopCountdown();
    setMsLoginUiState(false);
    setLoginMessage('Inicio de sesión cancelado.');
}

async function finishLogin(session) {
    const yaEstaba = ACCOUNTS.some(a => accountId(a) === accountId(session));
    await addAccount(session);

    if (usernameInput) usernameInput.value = '';
    setLoginMessage('');
    volverASelector = false;
    closeLoginModal();

    renderAccountUI();
    updateStatus(yaEstaba
        ? `Sesión renovada como ${session.username} (${tipoDeCuenta(session)})`
        : `Sesión iniciada como ${session.username} (${tipoDeCuenta(session)})`);
}

document.addEventListener('lumineria:require-login', () => {
    handleAccountButton();
});