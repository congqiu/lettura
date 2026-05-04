// Popup script

import { login, refreshToken, saveEntry } from '../shared/api';
import { getLocalStorage, setLocalStorage, getSessionStorage, setSessionStorage, clearAllStorage } from '../shared/storage';

// --- DOM Elements ---

const loginSection = document.getElementById('login-section')!;
const mainSection = document.getElementById('main-section')!;
const loadingSection = document.getElementById('loading-section')!;

const serverUrlInput = document.getElementById('server-url') as HTMLInputElement;
const emailInput = document.getElementById('email') as HTMLInputElement;
const passwordInput = document.getElementById('password') as HTMLInputElement;
const loginBtn = document.getElementById('login-btn') as HTMLButtonElement;
const loginError = document.getElementById('login-error')!;

const serverInfo = document.getElementById('server-info')!;
const saveBtn = document.getElementById('save-btn') as HTMLButtonElement;
const saveStatus = document.getElementById('save-status')!;
const logoutBtn = document.getElementById('logout-btn') as HTMLButtonElement;

// --- Helpers ---

function showSection(section: HTMLElement): void {
  loginSection.classList.add('hidden');
  mainSection.classList.add('hidden');
  loadingSection.classList.add('hidden');
  section.classList.remove('hidden');
}

function showMessage(el: HTMLElement, text: string, type: 'error' | 'success' | 'info'): void {
  el.textContent = text;
  el.className = `message ${type}`;
  el.classList.remove('hidden');
}

function hideMessage(el: HTMLElement): void {
  el.classList.add('hidden');
}

// --- Actions ---

async function doLogin(): Promise<void> {
  hideMessage(loginError);
  const serverUrl = serverUrlInput.value.trim();
  const email = emailInput.value.trim();
  const password = passwordInput.value;

  if (!serverUrl) {
    showMessage(loginError, 'Please enter the server URL.', 'error');
    return;
  }
  if (!serverUrl.startsWith('https://') && !serverUrl.startsWith('http://localhost') && !serverUrl.startsWith('http://127.0.0.1')) {
    showMessage(loginError, 'Server URL must use HTTPS (or http://localhost for development).', 'error');
    return;
  }
  if (!email) {
    showMessage(loginError, 'Please enter your email.', 'error');
    return;
  }
  if (!password) {
    showMessage(loginError, 'Please enter your password.', 'error');
    return;
  }

  loginBtn.disabled = true;
  loginBtn.textContent = 'Logging in...';

  try {
    await setLocalStorage({ server_url: serverUrl.replace(/\/+$/, '') });
    const tokens = await login(serverUrl, email, password);

    await setSessionStorage({ access_token: tokens.access_token });
    if (tokens.refresh_token) {
      await setLocalStorage({ refresh_token: tokens.refresh_token });
    }

    showMainSection(serverUrl);
  } catch (err) {
    showMessage(
      loginError,
      err instanceof Error ? err.message : 'Login failed',
      'error'
    );
  } finally {
    loginBtn.disabled = false;
    loginBtn.textContent = 'Login';
  }
}

async function doSave(): Promise<void> {
  hideMessage(saveStatus);
  saveBtn.disabled = true;
  saveBtn.textContent = 'Saving...';

  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!tab?.url) {
      showMessage(saveStatus, 'Cannot get current tab URL.', 'error');
      return;
    }

    const resp = await saveEntry(tab.url);

    if (resp.ok) {
      showMessage(saveStatus, 'Page saved successfully!', 'success');
    } else if (resp.status === 409) {
      showMessage(saveStatus, 'This page was already saved.', 'info');
    } else if (resp.status === 401) {
      showMessage(saveStatus, 'Session expired. Please login again.', 'error');
      await clearAllStorage();
      setTimeout(() => init(), 1000);
    } else {
      const errData = await resp.json().catch(() => null);
      const msg = errData?.message ?? `Failed to save (${resp.status})`;
      showMessage(saveStatus, msg, 'error');
    }
  } catch (err) {
    showMessage(
      saveStatus,
      err instanceof Error ? err.message : 'Failed to save page.',
      'error'
    );
  } finally {
    saveBtn.disabled = false;
    saveBtn.textContent = 'Save this page';
  }
}

async function doLogout(): Promise<void> {
  await clearAllStorage();
  showSection(loginSection);
  serverUrlInput.value = '';
  emailInput.value = '';
  passwordInput.value = '';
  hideMessage(loginError);
  hideMessage(saveStatus);
}

// --- UI state ---

function showMainSection(serverUrl: string): void {
  serverInfo.textContent = serverUrl;
  hideMessage(saveStatus);
  showSection(mainSection);
}

async function init(): Promise<void> {
  showSection(loadingSection);

  const { server_url, refresh_token } = await getLocalStorage(['server_url', 'refresh_token']);
  const { access_token } = await getSessionStorage(['access_token']);

  if (server_url && (access_token || refresh_token)) {
    // We have credentials — try to ensure we have a valid access token
    if (!access_token && refresh_token) {
      const newToken = await refreshToken();
      if (!newToken) {
        // Refresh failed, show login
        serverUrlInput.value = server_url ?? '';
        showSection(loginSection);
        return;
      }
    }
    showMainSection(server_url);
  } else {
    serverUrlInput.value = server_url ?? '';
    showSection(loginSection);
  }
}

// --- Event listeners ---

loginBtn.addEventListener('click', doLogin);
saveBtn.addEventListener('click', doSave);
logoutBtn.addEventListener('click', doLogout);

// Allow Enter key to submit login form
[serverUrlInput, emailInput, passwordInput].forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      doLogin();
    }
  });
});

// --- Init ---

init();
