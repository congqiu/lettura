// Popup script

import { login, refreshToken, saveEntry, connectWithToken } from '../shared/api';
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

// Tab elements
const tabPassword = document.getElementById('tab-password') as HTMLButtonElement;
const tabToken = document.getElementById('tab-token') as HTMLButtonElement;
const formPassword = document.getElementById('form-password')!;
const formToken = document.getElementById('form-token')!;

// Token form elements
const tokenServerUrlInput = document.getElementById('token-server-url') as HTMLInputElement;
const tokenInput = document.getElementById('token-input') as HTMLInputElement;
const tokenBtn = document.getElementById('token-btn') as HTMLButtonElement;
const tokenError = document.getElementById('token-error')!;

// Token badge in main section
const tokenBadge = document.getElementById('token-badge')!;

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

// --- Tab switching ---

function switchTab(tab: 'password' | 'token'): void {
  if (tab === 'password') {
    tabPassword.classList.add('active');
    tabToken.classList.remove('active');
    formPassword.classList.remove('hidden');
    formToken.classList.add('hidden');
    // Sync server URL from token form to password form
    if (tokenServerUrlInput.value) {
      serverUrlInput.value = tokenServerUrlInput.value;
    }
  } else {
    tabToken.classList.add('active');
    tabPassword.classList.remove('active');
    formToken.classList.remove('hidden');
    formPassword.classList.add('hidden');
    // Sync server URL from password form to token form
    if (serverUrlInput.value) {
      tokenServerUrlInput.value = serverUrlInput.value;
    }
  }
}

tabPassword.addEventListener('click', () => switchTab('password'));
tabToken.addEventListener('click', () => switchTab('token'));

// --- Actions ---

async function doLogin(): Promise<void> {
  hideMessage(loginError);
  const serverUrl = serverUrlInput.value.trim();
  const email = emailInput.value.trim();
  const password = passwordInput.value;

  if (!serverUrl) {
    showMessage(loginError, '请输入服务器地址', 'error');
    return;
  }
  if (!serverUrl.startsWith('https://') && !serverUrl.startsWith('http://localhost') && !serverUrl.startsWith('http://127.0.0.1')) {
    showMessage(loginError, '服务器地址必须使用 HTTPS（开发环境可用 http://localhost）', 'error');
    return;
  }
  if (!email) {
    showMessage(loginError, '请输入邮箱', 'error');
    return;
  }
  if (!password) {
    showMessage(loginError, '请输入密码', 'error');
    return;
  }

  loginBtn.disabled = true;
  loginBtn.textContent = '登录中...';

  try {
    await setLocalStorage({ server_url: serverUrl.replace(/\/+$/, '') });
    const tokens = await login(serverUrl, email, password);

    await setSessionStorage({ access_token: tokens.access_token });
    if (tokens.refresh_token) {
      await setLocalStorage({ refresh_token: tokens.refresh_token, auth_mode: 'jwt' });
    } else {
      await setLocalStorage({ auth_mode: 'jwt' });
    }

    showMainSection(serverUrl);
  } catch (err) {
    showMessage(
      loginError,
      err instanceof Error ? err.message : '登录失败',
      'error'
    );
  } finally {
    loginBtn.disabled = false;
    loginBtn.textContent = '登录';
  }
}

async function doTokenLogin(): Promise<void> {
  hideMessage(tokenError);
  const serverUrl = tokenServerUrlInput.value.trim();
  const token = tokenInput.value.trim();

  if (!serverUrl) {
    showMessage(tokenError, '请输入服务器地址', 'error');
    return;
  }
  if (!serverUrl.startsWith('https://') && !serverUrl.startsWith('http://localhost') && !serverUrl.startsWith('http://127.0.0.1')) {
    showMessage(tokenError, '服务器地址必须使用 HTTPS（开发环境可用 http://localhost）', 'error');
    return;
  }
  if (!token) {
    showMessage(tokenError, '请输入令牌', 'error');
    return;
  }
  if (!token.startsWith('lta_')) {
    showMessage(tokenError, '令牌必须以 lta_ 开头', 'error');
    return;
  }

  tokenBtn.disabled = true;
  tokenBtn.textContent = '连接中...';

  try {
    await connectWithToken(serverUrl, token);
    showMainSection(serverUrl, token);
  } catch (err) {
    showMessage(
      tokenError,
      err instanceof Error ? err.message : '连接失败',
      'error'
    );
  } finally {
    tokenBtn.disabled = false;
    tokenBtn.textContent = '连接';
  }
}

async function doSave(): Promise<void> {
  hideMessage(saveStatus);
  saveBtn.disabled = true;
  saveBtn.textContent = '保存中...';

  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!tab?.url) {
      showMessage(saveStatus, '无法获取当前页面地址', 'error');
      return;
    }

    const resp = await saveEntry(tab.url);

    if (resp.ok) {
      showMessage(saveStatus, '保存成功！', 'success');
    } else if (resp.status === 409) {
      showMessage(saveStatus, '该页面已保存过', 'info');
    } else if (resp.status === 401) {
      showMessage(saveStatus, '登录已过期，请重新登录', 'error');
      await clearAllStorage();
      setTimeout(() => init(), 1000);
    } else {
      const errData = await resp.json().catch(() => null);
      const msg = errData?.message ?? `保存失败 (${resp.status})`;
      showMessage(saveStatus, msg, 'error');
    }
  } catch (err) {
    showMessage(
      saveStatus,
      err instanceof Error ? err.message : '保存失败',
      'error'
    );
  } finally {
    saveBtn.disabled = false;
    saveBtn.textContent = '保存此页面';
  }
}

async function doLogout(): Promise<void> {
  await clearAllStorage();
  showSection(loginSection);
  switchTab('password');
  serverUrlInput.value = '';
  emailInput.value = '';
  passwordInput.value = '';
  tokenServerUrlInput.value = '';
  tokenInput.value = '';
  hideMessage(loginError);
  hideMessage(tokenError);
  hideMessage(saveStatus);
}

// --- UI state ---

function showMainSection(serverUrl: string, patToken?: string): void {
  serverInfo.textContent = serverUrl;
  hideMessage(saveStatus);

  if (patToken) {
    const prefix = patToken.substring(0, 12);
    tokenBadge.textContent = `${prefix}...`;
    tokenBadge.classList.remove('hidden');
  } else {
    tokenBadge.classList.add('hidden');
  }

  showSection(mainSection);
}

async function init(): Promise<void> {
  showSection(loadingSection);

  const { server_url, refresh_token, pat_token, auth_mode } = await getLocalStorage(['server_url', 'refresh_token', 'pat_token', 'auth_mode']);
  const { access_token } = await getSessionStorage(['access_token']);

  if (auth_mode === 'pat' && pat_token && server_url) {
    showMainSection(server_url, pat_token);
  } else if (auth_mode === 'jwt' && (access_token || refresh_token) && server_url) {
    // JWT mode — try to ensure we have a valid access token
    if (!access_token && refresh_token) {
      const newToken = await refreshToken();
      if (!newToken) {
        serverUrlInput.value = server_url ?? '';
        tokenServerUrlInput.value = server_url ?? '';
        showSection(loginSection);
        return;
      }
    }
    showMainSection(server_url);
  } else {
    serverUrlInput.value = server_url ?? '';
    tokenServerUrlInput.value = server_url ?? '';
    showSection(loginSection);
  }
}

// --- Event listeners ---

loginBtn.addEventListener('click', doLogin);
tokenBtn.addEventListener('click', doTokenLogin);
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

// Allow Enter key to submit token form
[tokenServerUrlInput, tokenInput].forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      doTokenLogin();
    }
  });
});

// --- Init ---

init();
