// popup.js — Lettura browser extension popup

(function () {
  "use strict";

  // --- DOM Elements ---
  const loginSection = document.getElementById("login-section");
  const mainSection = document.getElementById("main-section");
  const loadingSection = document.getElementById("loading-section");

  const serverUrlInput = document.getElementById("server-url");
  const emailInput = document.getElementById("email");
  const passwordInput = document.getElementById("password");
  const loginBtn = document.getElementById("login-btn");
  const loginError = document.getElementById("login-error");

  const serverInfo = document.getElementById("server-info");
  const saveBtn = document.getElementById("save-btn");
  const saveStatus = document.getElementById("save-status");
  const logoutBtn = document.getElementById("logout-btn");

  // --- Helpers ---

  function showSection(section) {
    loginSection.classList.add("hidden");
    mainSection.classList.add("hidden");
    loadingSection.classList.add("hidden");
    section.classList.remove("hidden");
  }

  function showMessage(el, text, type) {
    el.textContent = text;
    el.className = "message " + type;
    el.classList.remove("hidden");
  }

  function hideMessage(el) {
    el.classList.add("hidden");
  }

  function normalizeUrl(url) {
    // Remove trailing slash
    return url.replace(/\/+$/, "");
  }

  // --- Storage helpers ---

  async function getLocalStorage(keys) {
    return new Promise((resolve) => {
      chrome.storage.local.get(keys, resolve);
    });
  }

  async function setLocalStorage(data) {
    return new Promise((resolve) => {
      chrome.storage.local.set(data, resolve);
    });
  }

  async function getSessionStorage(keys) {
    return new Promise((resolve) => {
      chrome.storage.session.get(keys, resolve);
    });
  }

  async function setSessionStorage(data) {
    return new Promise((resolve) => {
      chrome.storage.session.set(data, resolve);
    });
  }

  async function clearAllStorage() {
    return new Promise((resolve) => {
      chrome.storage.local.remove(
        ["server_url", "refresh_token"],
        () => {
          chrome.storage.session.remove(["access_token"], resolve);
        }
      );
    });
  }

  // --- API calls ---

  async function apiRequest(method, path, body, accessToken) {
    const { server_url } = await getLocalStorage(["server_url"]);
    if (!server_url) {
      throw new Error("Server URL not configured");
    }

    const headers = { "Content-Type": "application/json" };
    if (accessToken) {
      headers["Authorization"] = "Bearer " + accessToken;
    }

    const resp = await fetch(normalizeUrl(server_url) + path, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    });

    return resp;
  }

  async function getAccessToken() {
    const { access_token } = await getSessionStorage(["access_token"]);
    return access_token || null;
  }

  /**
   * Make an authenticated API request. If a 401 is received, attempt
   * to refresh the token and retry once.
   */
  async function authenticatedRequest(method, path, body) {
    let token = await getAccessToken();
    if (!token) {
      // Try refreshing before giving up
      token = await refreshToken();
      if (!token) {
        throw new Error("Not authenticated");
      }
    }

    let resp = await apiRequest(method, path, body, token);

    if (resp.status === 401) {
      // Try refreshing
      const newToken = await refreshToken();
      if (!newToken) {
        throw new Error("Session expired. Please login again.");
      }
      resp = await apiRequest(method, path, body, newToken);
    }

    return resp;
  }

  async function refreshToken() {
    const { refresh_token } = await getLocalStorage(["refresh_token"]);
    if (!refresh_token) {
      return null;
    }

    try {
      const resp = await apiRequest("POST", "/api/auth/refresh", {
        refresh_token,
      });

      if (!resp.ok) {
        // Refresh failed — clear tokens
        await clearAllStorage();
        return null;
      }

      const data = await resp.json();
      await setSessionStorage({ access_token: data.access_token });
      if (data.refresh_token) {
        await setLocalStorage({ refresh_token: data.refresh_token });
      }
      return data.access_token;
    } catch (err) {
      console.error("Token refresh failed:", err);
      await clearAllStorage();
      return null;
    }
  }

  // --- Actions ---

  async function doLogin() {
    hideMessage(loginError);
    const serverUrl = serverUrlInput.value.trim();
    const email = emailInput.value.trim();
    const password = passwordInput.value;

    if (!serverUrl) {
      showMessage(loginError, "Please enter the server URL.", "error");
      return;
    }
    if (!email) {
      showMessage(loginError, "Please enter your email.", "error");
      return;
    }
    if (!password) {
      showMessage(loginError, "Please enter your password.", "error");
      return;
    }

    loginBtn.disabled = true;
    loginBtn.textContent = "Logging in...";

    try {
      await setLocalStorage({ server_url: normalizeUrl(serverUrl) });

      const resp = await apiRequest("POST", "/api/auth/login", {
        email,
        password,
      });

      if (!resp.ok) {
        const errData = await resp.json().catch(() => null);
        const msg =
          (errData && errData.message) ||
          "Login failed (HTTP " + resp.status + ")";
        showMessage(loginError, msg, "error");
        return;
      }

      const data = await resp.json();
      await setSessionStorage({ access_token: data.access_token });
      await setLocalStorage({ refresh_token: data.refresh_token });

      showMainSection(serverUrl);
    } catch (err) {
      showMessage(
        loginError,
        "Cannot connect to server. Check the URL and try again.",
        "error"
      );
    } finally {
      loginBtn.disabled = false;
      loginBtn.textContent = "Login";
    }
  }

  async function doSave() {
    hideMessage(saveStatus);
    saveBtn.disabled = true;
    saveBtn.textContent = "Saving...";

    try {
      const [tab] = await chrome.tabs.query({
        active: true,
        currentWindow: true,
      });
      if (!tab || !tab.url) {
        showMessage(saveStatus, "Cannot get current tab URL.", "error");
        return;
      }

      const resp = await authenticatedRequest("POST", "/api/entries", {
        url: tab.url,
      });

      if (resp.ok) {
        showMessage(saveStatus, "Page saved successfully!", "success");
      } else if (resp.status === 409) {
        showMessage(saveStatus, "This page was already saved.", "info");
      } else if (resp.status === 401) {
        showMessage(
          saveStatus,
          "Session expired. Please login again.",
          "error"
        );
        await clearAllStorage();
        setTimeout(() => init(), 1000);
      } else {
        const errData = await resp.json().catch(() => null);
        const msg =
          (errData && errData.message) ||
          "Failed to save (HTTP " + resp.status + ")";
        showMessage(saveStatus, msg, "error");
      }
    } catch (err) {
      showMessage(saveStatus, err.message || "Failed to save page.", "error");
    } finally {
      saveBtn.disabled = false;
      saveBtn.textContent = "Save this page";
    }
  }

  async function doLogout() {
    await clearAllStorage();
    showSection(loginSection);
    serverUrlInput.value = "";
    emailInput.value = "";
    passwordInput.value = "";
    hideMessage(loginError);
    hideMessage(saveStatus);
  }

  // --- UI state ---

  function showMainSection(serverUrl) {
    serverInfo.textContent = serverUrl;
    hideMessage(saveStatus);
    showSection(mainSection);
  }

  async function init() {
    showSection(loadingSection);

    const { server_url, refresh_token } = await getLocalStorage([
      "server_url",
      "refresh_token",
    ]);
    const { access_token } = await getSessionStorage(["access_token"]);

    if (server_url && (access_token || refresh_token)) {
      // We have credentials — try to ensure we have a valid access token
      if (!access_token && refresh_token) {
        const newToken = await refreshToken();
        if (!newToken) {
          // Refresh failed, show login
          serverUrlInput.value = server_url || "";
          showSection(loginSection);
          return;
        }
      }
      showMainSection(server_url);
    } else {
      serverUrlInput.value = server_url || "";
      showSection(loginSection);
    }
  }

  // --- Event listeners ---

  loginBtn.addEventListener("click", doLogin);
  saveBtn.addEventListener("click", doSave);
  logoutBtn.addEventListener("click", doLogout);

  // Allow Enter key to submit login form
  [serverUrlInput, emailInput, passwordInput].forEach((input) => {
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        doLogin();
      }
    });
  });

  // --- Init ---
  init();
})();
