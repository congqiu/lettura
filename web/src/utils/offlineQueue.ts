const QUEUE_KEY = 'lettura_offline_queue';

export interface OfflineEntry {
  url: string;
  createdAt: number;
}

export function getOfflineQueue(): OfflineEntry[] {
  try {
    return JSON.parse(localStorage.getItem(QUEUE_KEY) || '[]');
  } catch {
    return [];
  }
}

export function addToOfflineQueue(url: string) {
  const queue = getOfflineQueue();
  // Avoid duplicates
  if (queue.some((item) => item.url === url)) return;
  queue.push({ url, createdAt: Date.now() });
  localStorage.setItem(QUEUE_KEY, JSON.stringify(queue));
}

export function clearOfflineQueue() {
  localStorage.removeItem(QUEUE_KEY);
}

export function removeFromOfflineQueue(url: string) {
  const queue = getOfflineQueue().filter((item) => item.url !== url);
  if (queue.length === 0) {
    localStorage.removeItem(QUEUE_KEY);
  } else {
    localStorage.setItem(QUEUE_KEY, JSON.stringify(queue));
  }
}
