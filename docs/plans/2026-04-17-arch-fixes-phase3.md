# Phase 3: Frontend Quality Improvements

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate code duplication, fix hook performance issues, add theme system listener, and improve code consistency.

**Architecture:** Extract shared utilities (`timeAgo`, `useEntryActions`) into their own modules. Memoize keyboard shortcut hooks. Add `matchMedia` listener to theme store. Unify error handling patterns.

**Tech Stack:** TypeScript, React 19, TanStack Query, Zustand

**Depends on:** Phase 1 (DOMPurify already added)

---

## Task 1: Extract shared `timeAgo` utility

**Files:**
- Create: `web/src/utils/time.ts`
- Modify: `web/src/components/EntryCard.tsx`
- Modify: `web/src/components/PageCard.tsx`

- [ ] **Step 1: Create `web/src/utils/time.ts`**

```typescript
export function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}分钟前`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}小时前`;
  const days = Math.floor(hrs / 24);
  return `${days}天前`;
}
```

- [ ] **Step 2: Update `EntryCard.tsx`**

Remove the local `timeAgo` function (lines 7-15) and add:

```typescript
import { timeAgo } from '../utils/time';
```

- [ ] **Step 3: Update `PageCard.tsx`**

Remove the local `timeAgo` function (lines 6-14) and add:

```typescript
import { timeAgo } from '../utils/time';
```

- [ ] **Step 4: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 5: Commit**

```bash
git add web/src/utils/time.ts web/src/components/EntryCard.tsx web/src/components/PageCard.tsx
git commit -m "refactor: extract shared timeAgo utility"
```

---

## Task 2: Extract `useEntryActions` hook for shared star/archive logic

**Files:**
- Create: `web/src/hooks/useEntryActions.ts`
- Modify: `web/src/components/EntryCard.tsx`
- Modify: `web/src/pages/EntryDetailPage.tsx`

- [ ] **Step 1: Create `web/src/hooks/useEntryActions.ts`**

```typescript
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updateEntry } from '../api/entries';
import { toast } from '../components/Toast';

export function useEntryActions(
  entryId: string,
  entry: { is_starred: boolean; is_archived: boolean },
  extraInvalidation?: () => void,
) {
  const qc = useQueryClient();

  const toggleStar = useMutation({
    mutationFn: () => updateEntry(entryId, { is_starred: !entry.is_starred }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['entries'] });
      qc.invalidateQueries({ queryKey: ['entry', entryId] });
      toast('success', entry.is_starred ? '已取消收藏' : '已收藏');
      extraInvalidation?.();
    },
  });

  const toggleArchive = useMutation({
    mutationFn: () => updateEntry(entryId, { is_archived: !entry.is_archived }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['entries'] });
      qc.invalidateQueries({ queryKey: ['entry', entryId] });
      toast('success', entry.is_archived ? '已取消归档' : '已归档');
      extraInvalidation?.();
    },
  });

  return { toggleStar, toggleArchive };
}
```

- [ ] **Step 2: Update `EntryCard.tsx`**

Remove the inline `toggleStar` and `toggleArchive` mutations and replace with:

```typescript
import { useEntryActions } from '../hooks/useEntryActions';

// Inside the component:
const { toggleStar, toggleArchive } = useEntryActions(entry.id, entry);
```

- [ ] **Step 3: Update `EntryDetailPage.tsx`**

Remove the inline `toggleStar` and `toggleArchive` mutations and replace with:

```typescript
import { useEntryActions } from '../hooks/useEntryActions';

// Inside the component, after the entry query:
const { toggleStar, toggleArchive } = useEntryActions(
  id!,
  { is_starred: entry?.is_starred ?? false, is_archived: entry?.is_archived ?? false },
);
```

Note: `EntryDetailPage` has its own `invalidate()` function, but `useEntryActions` already handles invalidating both `['entries']` and `['entry', id]`, so the `extraInvalidation` callback is not needed.

- [ ] **Step 4: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 5: Commit**

```bash
git add web/src/hooks/useEntryActions.ts web/src/components/EntryCard.tsx web/src/pages/EntryDetailPage.tsx
git commit -m "refactor: extract useEntryActions hook to deduplicate star/archive logic"
```

---

## Task 3: Fix keyboard hooks performance — memoize with `useRef` and `useCallback`

**Files:**
- Modify: `web/src/hooks/useKeyboardShortcuts.ts`

- [ ] **Step 1: Memoize handlers with `useRef`**

```typescript
import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';

interface ShortcutHandlers {
  onStar?: () => void;
  onArchive?: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onBack?: () => void;
}

export function useKeyboardShortcuts(handlers: ShortcutHandlers = {}) {
  const navigate = useNavigate();
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement).isContentEditable) {
        return;
      }

      const h = handlersRef.current;
      switch (e.key) {
        case 's':
          e.preventDefault();
          h.onStar?.();
          break;
        case 'a':
          e.preventDefault();
          h.onArchive?.();
          break;
        case 'e':
          e.preventDefault();
          h.onEdit?.();
          break;
        case 'Backspace':
        case 'h':
          if (!e.metaKey && !e.ctrlKey) {
            h.onBack?.() || navigate(-1);
          }
          break;
        case '1':
          navigate('/');
          break;
        case '2':
          navigate('/archived');
          break;
        case '3':
          navigate('/starred');
          break;
        case '4':
          navigate('/memos');
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [navigate]);
}

export function useListKeyboardNav(
  entries: { id: string }[],
  selectedIndex: number,
  setSelectedIndex: (i: number) => void,
) {
  const navigate = useNavigate();
  const entriesRef = useRef(entries);
  entriesRef.current = entries;
  const selectedRef = useRef(selectedIndex);
  selectedRef.current = selectedIndex;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;

      const currentEntries = entriesRef.current;
      const currentSelected = selectedRef.current;

      switch (e.key) {
        case 'j':
          e.preventDefault();
          setSelectedIndex(Math.min(currentSelected + 1, currentEntries.length - 1));
          break;
        case 'k':
          e.preventDefault();
          setSelectedIndex(Math.max(currentSelected - 1, 0));
          break;
        case 'Enter':
        case 'o':
          if (currentEntries[currentSelected]) {
            navigate(`/entry/${currentEntries[currentSelected].id}`);
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [setSelectedIndex, navigate]);
}
```

- [ ] **Step 2: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add web/src/hooks/useKeyboardShortcuts.ts
git commit -m "fix: memoize keyboard shortcut hooks to prevent re-binding on every render"
```

---

## Task 4: Add system theme change listener to theme store

**Files:**
- Modify: `web/src/store/theme.ts`

- [ ] **Step 1: Add `matchMedia` change listener**

```typescript
import { create } from 'zustand';

type Theme = 'light' | 'dark' | 'system';

interface ThemeState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

function applyTheme(theme: Theme) {
  const isDark =
    theme === 'dark' ||
    (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.classList.toggle('dark', isDark);
}

export const useThemeStore = create<ThemeState>((set, get) => {
  const saved = (localStorage.getItem('theme') as Theme) || 'system';
  applyTheme(saved);

  if (typeof window !== 'undefined') {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (get().theme === 'system') {
        applyTheme('system');
      }
    };
    mediaQuery.addEventListener('change', handleChange);
  }

  return {
    theme: saved,
    setTheme: (theme) => {
      localStorage.setItem('theme', theme);
      applyTheme(theme);
      set({ theme });
    },
  };
});
```

- [ ] **Step 2: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add web/src/store/theme.ts
git commit -m "fix: add system theme change listener for auto dark mode updates"
```

---

## Task 5: Add Toast max count limit

**Files:**
- Modify: `web/src/components/Toast.tsx`

- [ ] **Step 1: Add max toast limit of 5**

In `Toast.tsx`, add a constant and enforce it:

```typescript
const MAX_TOASTS = 5;

export function toast(type: ToastType, message: string) {
  const id = String(nextId++);
  toasts = [...toasts, { id, type, message }];
  // Enforce max count — remove oldest extras
  while (toasts.length > MAX_TOASTS) {
    const oldest = toasts[0];
    const timer = timerMap.get(oldest.id);
    if (timer) {
      clearTimeout(timer);
      timerMap.delete(oldest.id);
    }
    toasts.shift();
  }
  emitChange();
  const timer = setTimeout(() => {
    timerMap.delete(id);
    toasts = toasts.filter((t) => t.id !== id);
    emitChange();
  }, 3000);
  timerMap.set(id, timer);
}
```

- [ ] **Step 2: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add web/src/components/Toast.tsx
git commit -m "fix: limit toast notifications to max 5 simultaneous"
```

---

## Task 6: Unify error handling in `EntryDetailPage` refetch mutation

**Files:**
- Modify: `web/src/pages/EntryDetailPage.tsx`

- [ ] **Step 1: Use `getErrorMessage` utility for refetch error**

Replace the manual error extraction:

```typescript
import { getErrorMessage } from '../utils/error';

// In the refetch mutation:
const refetch = useMutation({
  mutationFn: () => refetchEntry(id!),
  onSuccess: () => {
    invalidate();
    toast('success', '已加入重新抓取队列');
  },
  onError: (err: unknown) => {
    toast('error', getErrorMessage(err, '重新抓取失败'));
  },
});
```

- [ ] **Step 2: Run build**

Run: `cd web && pnpm run build`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add web/src/pages/EntryDetailPage.tsx
git commit -m "refactor: use getErrorMessage utility in refetch mutation"
```
