// Chrome Storage helpers with type safety

export async function getLocalStorage<K extends string>(
  keys: K[]
): Promise<Record<K, string | undefined>> {
  return new Promise((resolve) => {
    chrome.storage.local.get(keys, (result) => {
      resolve(result as Record<K, string | undefined>);
    });
  });
}

export async function setLocalStorage(data: Record<string, string>): Promise<void> {
  return new Promise((resolve) => {
    chrome.storage.local.set(data, resolve);
  });
}

export async function getSessionStorage<K extends string>(
  keys: K[]
): Promise<Record<K, string | undefined>> {
  return new Promise((resolve) => {
    chrome.storage.session.get(keys, (result) => {
      resolve(result as Record<K, string | undefined>);
    });
  });
}

export async function setSessionStorage(data: Record<string, string>): Promise<void> {
  return new Promise((resolve) => {
    chrome.storage.session.set(data, resolve);
  });
}

export async function clearAllStorage(): Promise<void> {
  return new Promise((resolve) => {
    chrome.storage.local.remove(['server_url', 'refresh_token'], () => {
      chrome.storage.session.remove(['access_token'], resolve);
    });
  });
}
