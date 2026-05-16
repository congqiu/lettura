import { useState, useEffect } from 'react';
import { login, refreshToken, saveEntry, connectWithToken } from '../shared/api';
import { getLocalStorage, getSessionStorage, clearAllStorage } from '../shared/storage';

type View = 'loading' | 'login' | 'main';
type SaveStatus = 'idle' | 'saving' | 'success' | 'duplicate' | 'error';

export default function App() {
  const [view, setView] = useState<View>('loading');
  const [serverUrl, setServerUrl] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [token, setToken] = useState('');
  const [loginTab, setLoginTab] = useState<'password' | 'token'>('password');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const [mainServerUrl, setMainServerUrl] = useState('');
  const [pageTitle, setPageTitle] = useState('');
  const [pageUrl, setPageUrl] = useState('');
  const [saveStatus, setSaveStatus] = useState<SaveStatus>('idle');
  const [saveMessage, setSaveMessage] = useState('');

  useEffect(() => {
    init();
  }, []);

  const init = async () => {
    const { server_url, refresh_token, pat_token, auth_mode } =
      await getLocalStorage(['server_url', 'refresh_token', 'pat_token', 'auth_mode']);
    const { access_token } = await getSessionStorage(['access_token']);

    if (server_url) setServerUrl(server_url);

    if (auth_mode === 'pat' && pat_token && server_url) {
      setMainServerUrl(server_url);
      await loadPageInfo();
      setView('main');
    } else if (auth_mode === 'jwt' && (access_token || refresh_token) && server_url) {
      if (!access_token && refresh_token) {
        const newToken = await refreshToken();
        if (!newToken) {
          setView('login');
          return;
        }
      }
      setMainServerUrl(server_url);
      await loadPageInfo();
      setView('main');
    } else {
      setView('login');
    }
  };

  const loadPageInfo = async () => {
    try {
      const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (tab) {
        setPageTitle(tab.title || '');
        setPageUrl(tab.url || '');
      }
    } catch {
      // ignore
    }
  };

  const doLogin = async () => {
    setError('');
    setLoading(true);
    try {
      if (!serverUrl) throw new Error('请输入服务器地址');
      if (!email) throw new Error('请输入邮箱');
      if (!password) throw new Error('请输入密码');

      await chrome.storage.local.set({ server_url: serverUrl.replace(/\/+$/, '') });
      const tokens = await login(serverUrl, email, password);

      await chrome.storage.session.set({ access_token: tokens.access_token });
      if (tokens.refresh_token) {
        await chrome.storage.local.set({ refresh_token: tokens.refresh_token, auth_mode: 'jwt' });
      } else {
        await chrome.storage.local.set({ auth_mode: 'jwt' });
      }

      setMainServerUrl(serverUrl);
      await loadPageInfo();
      setView('main');
    } catch (err: any) {
      setError(err.message || '登录失败');
    } finally {
      setLoading(false);
    }
  };

  const doTokenLogin = async () => {
    setError('');
    setLoading(true);
    try {
      if (!serverUrl) throw new Error('请输入服务器地址');
      if (!token) throw new Error('请输入令牌');
      if (!token.startsWith('lta_')) throw new Error('令牌必须以 lta_ 开头');

      await connectWithToken(serverUrl, token);
      setMainServerUrl(serverUrl);
      await loadPageInfo();
      setView('main');
    } catch (err: any) {
      setError(err.message || '连接失败');
    } finally {
      setLoading(false);
    }
  };

  const doSave = async () => {
    setSaveStatus('saving');
    setSaveMessage('');
    try {
      const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (!tab?.url) {
        setSaveStatus('error');
        setSaveMessage('无法获取当前页面地址');
        return;
      }

      const resp = await saveEntry(tab.url);

      if (resp.ok) {
        setSaveStatus('success');
        setSaveMessage('保存成功');
      } else if (resp.status === 409) {
        setSaveStatus('duplicate');
        setSaveMessage('该页面已保存过');
      } else if (resp.status === 401) {
        setSaveStatus('error');
        setSaveMessage('登录已过期，请重新登录');
        await clearAllStorage();
        setView('login');
      } else {
        const errData = await resp.json().catch(() => null);
        setSaveStatus('error');
        setSaveMessage(errData?.message ?? `保存失败 (${resp.status})`);
      }
    } catch (err: any) {
      setSaveStatus('error');
      setSaveMessage(err.message || '保存失败');
    }
  };

  const doLogout = async () => {
    await clearAllStorage();
    setServerUrl('');
    setEmail('');
    setPassword('');
    setToken('');
    setSaveStatus('idle');
    setSaveMessage('');
    setView('login');
  };

  if (view === 'loading') {
    return (
      <div className="flex items-center justify-center py-10">
        <div className="spinner" />
      </div>
    );
  }

  if (view === 'login') {
    return (
      <div className="p-4">
        <header className="flex items-center gap-2.5 mb-5 pb-3 border-b border-border">
          <div className="logo"><span>L</span></div>
          <h1 className="text-base font-semibold">Lettura</h1>
        </header>

        <div className="tabs mb-5">
          <button
            onClick={() => setLoginTab('password')}
            className={`tab ${loginTab === 'password' ? 'active' : ''}`}
          >
            密码登录
          </button>
          <button
            onClick={() => setLoginTab('token')}
            className={`tab ${loginTab === 'token' ? 'active' : ''}`}
          >
            令牌登录
          </button>
        </div>

        {error && <div className="message error mb-3">{error}</div>}

        {loginTab === 'password' ? (
          <div className="section gap-3">
            <div className="form-group">
              <label>服务器地址</label>
              <input
                type="url"
                value={serverUrl}
                onChange={(e) => setServerUrl(e.target.value)}
                placeholder="https://lettura.example.com"
                onKeyDown={(e) => { if (e.key === 'Enter') doLogin(); }}
              />
            </div>
            <div className="form-group">
              <label>邮箱</label>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                onKeyDown={(e) => { if (e.key === 'Enter') doLogin(); }}
              />
            </div>
            <div className="form-group">
              <label>密码</label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="输入密码"
                onKeyDown={(e) => { if (e.key === 'Enter') doLogin(); }}
              />
            </div>
            <button
              onClick={doLogin}
              disabled={loading}
              className="btn btn-primary"
            >
              {loading ? '登录中...' : '登录'}
            </button>
          </div>
        ) : (
          <div className="section gap-3">
            <div className="form-group">
              <label>服务器地址</label>
              <input
                type="url"
                value={serverUrl}
                onChange={(e) => setServerUrl(e.target.value)}
                placeholder="https://lettura.example.com"
                onKeyDown={(e) => { if (e.key === 'Enter') doTokenLogin(); }}
              />
            </div>
            <div className="form-group">
              <label>API 令牌</label>
              <input
                type="text"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="lta_..."
                onKeyDown={(e) => { if (e.key === 'Enter') doTokenLogin(); }}
              />
            </div>
            <button
              onClick={doTokenLogin}
              disabled={loading}
              className="btn btn-primary"
            >
              {loading ? '连接中...' : '连接'}
            </button>
          </div>
        )}
      </div>
    );
  }

  // Main view
  return (
    <div className="p-4">
      <header className="flex items-center gap-2.5 mb-4 pb-3 border-b border-border">
        <div className="logo"><span>L</span></div>
        <div className="min-w-0 flex-1">
          <h1 className="text-base font-semibold">Lettura</h1>
          <p className="server-info">{mainServerUrl}</p>
        </div>
      </header>

      {pageUrl && (
        <div className="page-preview mb-4">
          <div className="page-preview-text">
            <p className="page-preview-title">{pageTitle || pageUrl}</p>
            <p className="page-preview-url">{pageUrl}</p>
          </div>
        </div>
      )}

      <button
        onClick={doSave}
        disabled={saveStatus === 'saving'}
        className="btn btn-primary mb-3"
      >
        {saveStatus === 'saving' ? '保存中...' : '保存此页面'}
      </button>

      {saveStatus !== 'idle' && saveStatus !== 'saving' && (
        <div className={`message mb-3 ${
          saveStatus === 'success' ? 'success' :
          saveStatus === 'duplicate' ? 'info' : 'error'
        }`}>
          {saveMessage}
        </div>
      )}

      <button onClick={doLogout} className="btn btn-secondary w-full">
        退出登录
      </button>
    </div>
  );
}
