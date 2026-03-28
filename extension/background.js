// background.js — Lettura browser extension service worker

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
    chrome.storage.local.remove(["server_url", "refresh_token"], () => {
      chrome.storage.session.remove(["access_token"], resolve);
    });
  });
}

// --- API helpers ---

function normalizeUrl(url) {
  return url.replace(/\/+$/, "");
}

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

async function refreshToken() {
  const { refresh_token } = await getLocalStorage(["refresh_token"]);
  if (!refresh_token) {
    return null;
  }

  try {
    const resp = await apiRequest("POST", "/api/v1/auth/refresh", {
      refresh_token,
    });

    if (!resp.ok) {
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

/**
 * Make an authenticated API request with automatic token refresh on 401.
 */
async function authenticatedRequest(method, path, body) {
  let token = await getAccessToken();
  if (!token) {
    token = await refreshToken();
    if (!token) {
      throw new Error("Not authenticated. Please login via the popup.");
    }
  }

  let resp = await apiRequest(method, path, body, token);

  if (resp.status === 401) {
    const newToken = await refreshToken();
    if (!newToken) {
      throw new Error("Session expired. Please login again via the popup.");
    }
    resp = await apiRequest(method, path, body, newToken);
  }

  return resp;
}

// --- Notification helper ---

function notify(title, message) {
  // Use the badge on the action icon as a simple notification mechanism.
  // We set the badge text briefly then clear it.
  chrome.action.setBadgeText({ text: "!" });
  chrome.action.setBadgeBackgroundColor({ color: "#4361ee" });
  setTimeout(() => {
    chrome.action.setBadgeText({ text: "" });
  }, 3000);

  // Also log for debugging
  console.log("[Lettura]", title, "-", message);
}

// --- Context menus ---

chrome.runtime.onInstalled.addListener(() => {
  // "Save to Lettura" — available on all pages
  chrome.contextMenus.create({
    id: "save-page",
    title: "Save to Lettura",
    contexts: ["page", "link"],
  });

  // "Save as Memo" — available when text is selected
  chrome.contextMenus.create({
    id: "save-memo",
    title: "Save as Memo",
    contexts: ["selection"],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === "save-page") {
    await handleSavePage(info, tab);
  } else if (info.menuItemId === "save-memo") {
    await handleSaveMemo(info, tab);
  }
});

async function handleSavePage(info, tab) {
  // If right-clicked on a link, save that link's URL; otherwise save the page URL
  const url = info.linkUrl || (tab && tab.url);
  if (!url) {
    notify("Error", "Cannot determine URL to save.");
    return;
  }

  try {
    const resp = await authenticatedRequest("POST", "/api/v1/entries", { url });

    if (resp.ok) {
      chrome.action.setBadgeText({ text: "OK" });
      chrome.action.setBadgeBackgroundColor({ color: "#27ae60" });
    } else if (resp.status === 409) {
      chrome.action.setBadgeText({ text: "DUP" });
      chrome.action.setBadgeBackgroundColor({ color: "#f39c12" });
    } else {
      chrome.action.setBadgeText({ text: "ERR" });
      chrome.action.setBadgeBackgroundColor({ color: "#e74c3c" });
    }

    setTimeout(() => {
      chrome.action.setBadgeText({ text: "" });
    }, 3000);
  } catch (err) {
    console.error("[Lettura] Save page failed:", err);
    chrome.action.setBadgeText({ text: "ERR" });
    chrome.action.setBadgeBackgroundColor({ color: "#e74c3c" });
    setTimeout(() => {
      chrome.action.setBadgeText({ text: "" });
    }, 3000);
  }
}

async function handleSaveMemo(info, tab) {
  const selectedText = info.selectionText;
  if (!selectedText) {
    notify("Error", "No text selected.");
    return;
  }

  const sourceUrl = (tab && tab.url) || "";

  try {
    const resp = await authenticatedRequest("POST", "/api/v1/memos", {
      content: selectedText,
      source_url: sourceUrl,
    });

    if (resp.ok) {
      chrome.action.setBadgeText({ text: "OK" });
      chrome.action.setBadgeBackgroundColor({ color: "#27ae60" });
    } else {
      chrome.action.setBadgeText({ text: "ERR" });
      chrome.action.setBadgeBackgroundColor({ color: "#e74c3c" });
    }

    setTimeout(() => {
      chrome.action.setBadgeText({ text: "" });
    }, 3000);
  } catch (err) {
    console.error("[Lettura] Save memo failed:", err);
    chrome.action.setBadgeText({ text: "ERR" });
    chrome.action.setBadgeBackgroundColor({ color: "#e74c3c" });
    setTimeout(() => {
      chrome.action.setBadgeText({ text: "" });
    }, 3000);
  }
}
