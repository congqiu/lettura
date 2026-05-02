// Background service worker

import { saveEntry, createMemo } from '../shared/api';
import type { BadgeStatus } from '../shared/types';

// --- Badge notification ---

function setBadge(status: BadgeStatus, color: string): void {
  chrome.action.setBadgeText({ text: status });
  chrome.action.setBadgeBackgroundColor({ color });
  setTimeout(() => {
    chrome.action.setBadgeText({ text: '' });
  }, 3000);
}

// --- Context menus ---

chrome.runtime.onInstalled.addListener(() => {
  // "Save to Lettura" — available on all pages
  chrome.contextMenus.create({
    id: 'save-page',
    title: 'Save to Lettura',
    contexts: ['page', 'link'],
  });

  // "Save as Memo" — available when text is selected
  chrome.contextMenus.create({
    id: 'save-memo',
    title: 'Save as Memo',
    contexts: ['selection'],
  });
});

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (info.menuItemId === 'save-page') {
    await handleSavePage(info, tab);
  } else if (info.menuItemId === 'save-memo') {
    await handleSaveMemo(info, tab);
  }
});

async function handleSavePage(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab
): Promise<void> {
  // If right-clicked on a link, save that link's URL; otherwise save the page URL
  const url = info.linkUrl ?? tab?.url;
  if (!url) {
    setBadge('ERR', '#e74c3c');
    return;
  }

  try {
    const resp = await saveEntry(url);

    if (resp.ok) {
      setBadge('OK', '#27ae60');
    } else if (resp.status === 409) {
      setBadge('DUP', '#f39c12');
    } else {
      setBadge('ERR', '#e74c3c');
    }
  } catch (err) {
    console.error('[Lettura] Save page failed:', err);
    setBadge('ERR', '#e74c3c');
  }
}

async function handleSaveMemo(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab
): Promise<void> {
  const selectedText = info.selectionText;
  if (!selectedText) {
    setBadge('ERR', '#e74c3c');
    return;
  }

  const sourceUrl = tab?.url ?? '';

  try {
    const resp = await createMemo(selectedText, sourceUrl);

    if (resp.ok) {
      setBadge('OK', '#27ae60');
    } else {
      setBadge('ERR', '#e74c3c');
    }
  } catch (err) {
    console.error('[Lettura] Save memo failed:', err);
    setBadge('ERR', '#e74c3c');
  }
}
