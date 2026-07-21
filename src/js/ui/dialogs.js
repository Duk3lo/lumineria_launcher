let resolver = null;

export function initDialogs() {
    const overlay = document.getElementById('app-dialog-overlay');
    const okBtn = document.getElementById('app-dialog-ok');
    const cancelBtn = document.getElementById('app-dialog-cancel');

    okBtn.addEventListener('click', () => closeDialog(true));
    cancelBtn.addEventListener('click', () => closeDialog(false));
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) closeDialog(false);
    });
}

function closeDialog(result) {
    document.getElementById('app-dialog-overlay').classList.add('hidden');
    if (resolver) resolver(result);
    resolver = null;
}

function openDialog({ title, message, showCancel }) {
    document.getElementById('app-dialog-title').innerText = title;
    document.getElementById('app-dialog-message').innerText = message;
    document.getElementById('app-dialog-cancel').style.display = showCancel ? 'inline-block' : 'none';
    document.getElementById('app-dialog-overlay').classList.remove('hidden');

    return new Promise((resolve) => { resolver = resolve; });
}

export function showAlert(message, title = 'Aviso') {
    return openDialog({ title, message, showCancel: false });
}

export function showConfirm(message, title = 'Confirmar') {
    return openDialog({ title, message, showCancel: true });
}
