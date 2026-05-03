# 移动端体验优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复移动端 bug 并增强触摸手势体验，使 Lettura 在手机上可用、好用。

**Architecture:** 前端纯 React + Tailwind 修改，不涉及后端改动。手势通过通用 `useSwipe` hook 实现，移动/桌面分支用 `useIsMobile` hook + `lg:` 断点。

**Tech Stack:** React 19, TypeScript, Tailwind CSS v4, shadcn/ui (Sheet), react-router-dom, @tanstack/react-query, vite-plugin-pwa

**Spec:** `docs/superpowers/specs/2026-05-03-mobile-ux-optimization-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `web/src/pages/EntryDetailPage.tsx` | Modify | 修复动态 Tailwind 类名，AnnotationsSidebar 移动端 Sheet，手势集成 |
| `web/src/components/AnnotationsSidebar.tsx` | Modify | 添加 `compact` prop，移动端去掉 `w-80 border-l` |
| `web/src/pages/EntryListPage.tsx` | Modify | 批量操作栏位置修复，选择框移动端布局，下拉刷新 |
| `web/src/components/EntryCard.tsx` | Modify | 选择框移入组件，导航时传递列表上下文 |
| `web/src/components/Layout.tsx` | Modify | safe-area-inset-top |
| `web/src/components/layout/MobileBottomNav.tsx` | Modify | 设置 CSS 变量 `--bottom-nav-height` |
| `web/src/pages/SettingsPage.tsx` | Modify | 标签管理表格移动端卡片布局 |
| `web/src/pages/PagesPage.tsx` | Modify | tab 栏横向滚动 |
| `web/src/pages/LoginPage.tsx` | Modify | 支持 `redirect` 查询参数 |
| `web/src/App.tsx` | Modify | 添加 `/share-target` 路由 |
| `web/src/pages/ShareTargetPage.tsx` | Create | Web Share Target 接收页面 |
| `web/src/hooks/useSwipe.ts` | Create | 通用滑动手势 hook |
| `web/src/hooks/__tests__/useSwipe.test.ts` | Create | useSwipe 单元测试 |
| `web/src/hooks/use-mobile.ts` | Modify (无) | 已有，不改动 |
| `web/src/hooks/useKeyboardShortcuts.ts` | Modify | 导航时传递列表上下文 |
| `web/vite.config.ts` | Modify | 添加 `share_target` 到 PWA manifest |
| `web/index.html` | Modify | 添加 `viewport-fit=cover` |
| `web/src/index.css` | Modify | 添加 `scrollbar-hide` 工具类 |

---

### Task 1: M2 — 修复 EntryDetailPage 动态 Tailwind 类名

**Files:**
- Modify: `web/src/pages/EntryDetailPage.tsx:75`

- [ ] **Step 1: 修复动态类名**

将第 75 行：
```tsx
<div className={`flex-1 px-4 w-full overflow-hidden lg:${showAnnotations ? 'max-w-3xl' : 'max-w-3xl'} ${!showAnnotations ? 'lg:mx-auto' : ''}`}>
```
替换为：
```tsx
<div className={`flex-1 px-4 w-full overflow-hidden lg:max-w-3xl ${!showAnnotations ? 'lg:mx-auto' : ''}`}>
```

- [ ] **Step 2: 全局搜索其他动态拼接模式**

Run: `grep -rn 'lg:\${\|md:\${\|sm:\${\|xl:\${' web/src/`

Expected: 无其他匹配（或记录并修复找到的）

- [ ] **Step 3: 验证构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -5"`

Expected: 构建成功

- [ ] **Step 4: Commit**

```bash
git add web/src/pages/EntryDetailPage.tsx
git commit -m "fix(mobile): replace dynamic Tailwind class with static class in EntryDetailPage"
```

---

### Task 2: M6 — safe-area-inset-top

**Files:**
- Modify: `web/index.html:6`
- Modify: `web/src/components/Layout.tsx:17`

- [ ] **Step 1: 添加 viewport-fit=cover**

将 `web/index.html` 第 6 行：
```html
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
```
替换为：
```html
<meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />
```

- [ ] **Step 2: 移动端 header 添加 safe-area-inset-top**

将 `web/src/components/Layout.tsx` 第 17 行：
```tsx
<header className="flex h-14 items-center gap-2 border-b border-border bg-card px-4 lg:hidden">
```
替换为：
```tsx
<header className="flex h-14 items-center gap-2 border-b border-border bg-card px-4 pt-[env(safe-area-inset-top)] lg:hidden">
```

- [ ] **Step 3: Commit**

```bash
git add web/index.html web/src/components/Layout.tsx
git commit -m "fix(mobile): add viewport-fit=cover and safe-area-inset-top for iPhone notch"
```

---

### Task 3: M8 — 选择框裁切修复

**Files:**
- Modify: `web/src/components/EntryCard.tsx`
- Modify: `web/src/pages/EntryListPage.tsx:282-301`

- [ ] **Step 1: 给 EntryCard 添加选择框 props**

在 `web/src/components/EntryCard.tsx` 中，修改 Props 类型和组件：

```tsx
export default function EntryCard({
  entry,
  selected = false,
  onDomainClick,
  selectionMode = false,
  entrySelected = false,
  onToggleSelect,
}: {
  entry: EntrySummary;
  selected?: boolean;
  onDomainClick?: (domain: string) => void;
  selectionMode?: boolean;
  entrySelected?: boolean;
  onToggleSelect?: () => void;
}) {
```

在组件返回的 `<div className={cn(...)}>` 内部最前面添加选择框：

```tsx
{selectionMode && (
  <button
    onClick={(e) => { e.preventDefault(); onToggleSelect?.(); }}
    className="shrink-0 text-muted-foreground hover:text-foreground transition-colors mr-2"
  >
    {entrySelected ? (
      <CheckSquare size={18} className="text-primary" />
    ) : (
      <Square size={18} />
    )}
  </button>
)}
```

添加 import：
```tsx
import { Star, Archive, ExternalLink, Clock, CheckSquare, Square } from 'lucide-react';
```

- [ ] **Step 2: 从 EntryListPage 移除旧的选择框**

在 `web/src/pages/EntryListPage.tsx` 中，删除第 284-295 行的旧选择框代码：
```tsx
{selectionMode && (
  <button
    onClick={() => toggleSelect(entry.id)}
    className="absolute left-0 top-5 z-10 -ml-8 text-muted-foreground hover:text-foreground transition-colors"
  >
    {selectedIds.has(entry.id) ? (
      <CheckSquare size={18} className="text-primary" />
    ) : (
      <Square size={18} />
    )}
  </button>
)}
```

将 `<div key={entry.id} className="relative">` 改为 `<div key={entry.id}>`。

修改 EntryCard 调用，传入新 props：
```tsx
<EntryCard
  entry={entry}
  selected={i === selectedIndex || selectedIds.has(entry.id)}
  onDomainClick={(d) => setDomain(d)}
  selectionMode={selectionMode}
  entrySelected={selectedIds.has(entry.id)}
  onToggleSelect={() => toggleSelect(entry.id)}
/>
```

从 EntryListPage import 中移除 `CheckSquare, Square`（如果不再使用）。

- [ ] **Step 3: 验证构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -5"`

- [ ] **Step 4: Commit**

```bash
git add web/src/components/EntryCard.tsx web/src/pages/EntryListPage.tsx
git commit -m "fix(mobile): move selection checkbox into EntryCard for mobile layout"
```

---

### Task 4: M7 — Pages 页 tab 栏滚动

**Files:**
- Modify: `web/src/index.css`
- Modify: `web/src/pages/PagesPage.tsx:31`

- [ ] **Step 1: 添加 scrollbar-hide 工具类**

在 `web/src/index.css` 的 `@layer base` 块之后添加：

```css
@utility scrollbar-hide {
  scrollbar-width: none;
  &::-webkit-scrollbar {
    display: none;
  }
}
```

- [ ] **Step 2: tab 栏添加横向滚动**

在 `web/src/pages/PagesPage.tsx` 中，将第 31 行：
```tsx
<div className="flex gap-1">
```
替换为：
```tsx
<div className="flex gap-1 overflow-x-auto flex-nowrap scrollbar-hide">
```

- [ ] **Step 3: Commit**

```bash
git add web/src/index.css web/src/pages/PagesPage.tsx
git commit -m "fix(mobile): add horizontal scroll to Pages tab bar on narrow screens"
```

---

### Task 5: M3 — 批量操作栏与底部导航重叠

**Files:**
- Modify: `web/src/components/layout/MobileBottomNav.tsx`
- Modify: `web/src/pages/EntryListPage.tsx:325`

- [ ] **Step 1: MobileBottomNav 设置 CSS 变量**

在 `web/src/components/layout/MobileBottomNav.tsx` 中，在组件内添加 `useLayoutEffect` 测量高度：

```tsx
import { useState, useLayoutEffect, useRef } from 'react';
```

在组件函数体内：
```tsx
const navRef = useRef<HTMLDivElement>(null);

useLayoutEffect(() => {
  const el = navRef.current;
  if (el) {
    document.documentElement.style.setProperty('--bottom-nav-height', `${el.offsetHeight}px`);
  }
}, []);
```

将外层 `<div className="fixed bottom-0 inset-x-0 z-40 border-t border-border bg-card lg:hidden">` 添加 ref：
```tsx
<div ref={navRef} className="fixed bottom-0 inset-x-0 z-40 border-t border-border bg-card lg:hidden">
```

- [ ] **Step 2: 批量操作栏使用 CSS 变量定位**

在 `web/src/pages/EntryListPage.tsx` 中，将第 325 行：
```tsx
<div className="fixed bottom-0 left-0 right-0 z-50 bg-background border-t border-border shadow-lg">
```
替换为（桌面端依赖 CSS 变量 fallback 到 `0px`，移动端由 CSS 变量控制）：
```tsx
<div className="fixed left-0 right-0 z-50 bg-background border-t border-border shadow-lg pb-[env(safe-area-inset-bottom)]" style={{ bottom: 'var(--bottom-nav-height, 0px)' }}>
```

- [ ] **Step 3: Commit**

```bash
git add web/src/components/layout/MobileBottomNav.tsx web/src/pages/EntryListPage.tsx
git commit -m "fix(mobile): position bulk action bar above bottom nav using CSS variable"
```

---

### Task 6: M4 — Settings 页标签管理表格移动端卡片布局

**Files:**
- Modify: `web/src/pages/SettingsPage.tsx:239-294`

- [ ] **Step 1: 添加移动端卡片布局**

在 `web/src/pages/SettingsPage.tsx` 中，将标签管理表格部分（第 239-294 行）替换为同时包含表格和卡片的布局：

```tsx
<div className="border border-border rounded-lg overflow-hidden hidden sm:block">
  <table className="w-full text-sm">
    {/* 现有 thead/tbody 不变 */}
  </table>
</div>
<div className="space-y-2 sm:hidden">
  {tagStats.map((tag) => (
    <div key={tag.id} className="border border-border rounded-lg p-3 bg-card">
      <div className="flex items-center justify-between mb-2">
        {editingTagId === tag.id ? (
          <Input
            value={editingLabel}
            onChange={(e) => setEditingLabel(e.target.value)}
            onKeyDown={(e) => handleRenameKeyDown(e, tag.id)}
            onBlur={() => setEditingTagId(null)}
            className="h-7 text-sm flex-1 mr-2"
            autoFocus
          />
        ) : (
          <span className="font-medium text-card-foreground">{tag.label}</span>
        )}
        <span className="text-sm text-muted-foreground">{tag.entry_count} 篇</span>
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2"
          onClick={() => { setEditingTagId(tag.id); setEditingLabel(tag.label); }}
        >
          <Pencil size={14} className="mr-1" /> 编辑
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2 hover:text-destructive"
          onClick={() => setDeleteTarget({ id: tag.id, label: tag.label })}
        >
          <Trash2 size={14} className="mr-1" /> 删除
        </Button>
      </div>
    </div>
  ))}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add web/src/pages/SettingsPage.tsx
git commit -m "feat(mobile): add card layout for tag management table on small screens"
```

---

### Task 7: M1 — AnnotationsSidebar 移动端适配

**Files:**
- Modify: `web/src/components/AnnotationsSidebar.tsx:10,50`
- Modify: `web/src/pages/EntryDetailPage.tsx:145-152,197`

- [ ] **Step 1: AnnotationsSidebar 添加 compact prop**

在 `web/src/components/AnnotationsSidebar.tsx` 中：

修改 Props：
```tsx
interface Props { entryId: string; compact?: boolean; }
```

修改第 50 行的外层 div：
```tsx
<div className={compact ? 'bg-card p-4 overflow-y-auto' : 'w-80 border-l border-border bg-card p-4 overflow-y-auto'}>
```

- [ ] **Step 2: EntryDetailPage 移动端 Sheet 渲染**

在 `web/src/pages/EntryDetailPage.tsx` 中：

添加 import：
```tsx
import { useIsMobile } from '../hooks/use-mobile';
import { Sheet, SheetContent } from '@/components/ui/sheet';
```

在组件内添加：
```tsx
const isMobile = useIsMobile();
```

修改第 145-152 行的批注按钮（保持一个按钮，移动端打开 Sheet，桌面端切换侧栏，由 `showAnnotations` 状态统一控制）：
无需修改按钮代码，按钮本身不变。

修改第 197 行的 AnnotationsSidebar 渲染，替换为：
```tsx
{isMobile ? (
  <Sheet open={showAnnotations} onOpenChange={setShowAnnotations}>
    <SheetContent side="bottom" className="h-[60dvh]">
      {id && <AnnotationsSidebar entryId={id} compact />}
    </SheetContent>
  </Sheet>
) : (
  showAnnotations && id && <AnnotationsSidebar entryId={id} />
)}
```

- [ ] **Step 3: 验证构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -5"`

- [ ] **Step 4: Commit**

```bash
git add web/src/components/AnnotationsSidebar.tsx web/src/pages/EntryDetailPage.tsx
git commit -m "feat(mobile): render AnnotationsSidebar as bottom Sheet on mobile"
```

---

### Task 8: M5 — Web Share Target API

**Files:**
- Modify: `web/vite.config.ts`
- Create: `web/src/pages/ShareTargetPage.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/pages/LoginPage.tsx`

- [ ] **Step 1: vite.config.ts 添加 share_target**

在 `web/vite.config.ts` 的 `VitePWA` 配置的 `manifest` 对象中添加：
```ts
share_target: {
  action: '/share-target',
  method: 'GET',
  enctype: 'application/x-www-form-urlencoded',
  params: {
    url: 'url',
    text: 'text',
  },
},
```

- [ ] **Step 2: 创建 ShareTargetPage.tsx**

创建 `web/src/pages/ShareTargetPage.tsx`：

```tsx
import { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { createEntry } from '../api/entries';
import { useAuthStore } from '../store/auth';
import { Button } from '@/components/ui/button';
import { Loader2, CheckCircle2, XCircle } from 'lucide-react';

const URL_REGEX = /https?:\/\/[^\s<>"{}|\\^`\[\]]+/;

function extractUrl(urlParam: string | null, textParam: string | null): string | null {
  if (urlParam && URL_REGEX.test(urlParam)) return urlParam.match(URL_REGEX)![0];
  if (textParam) {
    const match = textParam.match(URL_REGEX);
    if (match) return match[0];
  }
  return null;
}

export default function ShareTargetPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { isAuthenticated } = useAuthStore();
  const [status, setStatus] = useState<'loading' | 'success' | 'error' | 'no-url'>('loading');
  const [savedEntryId, setSavedEntryId] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState('');

  useEffect(() => {
    if (!isAuthenticated) {
      const currentUrl = window.location.pathname + window.location.search;
      sessionStorage.setItem('lettura_share_redirect', currentUrl);
      navigate('/login?redirect=' + encodeURIComponent(currentUrl));
      return;
    }

    const urlParam = searchParams.get('url');
    const textParam = searchParams.get('text');
    handleSave(urlParam, textParam);
  }, []);

  const handleSave = async (urlParam: string | null, textParam: string | null) => {
    const url = extractUrl(urlParam, textParam);
    if (!url) {
      setStatus('no-url');
      return;
    }
    try {
      const entry = await createEntry(url);
      setStatus('success');
      setSavedEntryId(entry.id);
      setTimeout(() => navigate(`/entry/${entry.id}`), 2000);
    } catch (err: any) {
      if (err.response?.status === 409) {
        setStatus('success');
        const existingId = err.response?.data?.id;
        setSavedEntryId(existingId || null);
        if (existingId) setTimeout(() => navigate(`/entry/${existingId}`), 2000);
      } else {
        setStatus('error');
        setErrorMsg(err.response?.data?.message || '保存失败');
      }
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-background p-4">
      <div className="w-full max-w-sm p-8 bg-card border border-border rounded-xl shadow-sm text-center">
        {status === 'loading' && (
          <>
            <Loader2 size={32} className="animate-spin mx-auto mb-4 text-primary" />
            <p className="text-foreground">正在保存...</p>
          </>
        )}
        {status === 'success' && (
          <>
            <CheckCircle2 size={32} className="mx-auto mb-4 text-green-500" />
            <p className="text-foreground mb-2">已保存</p>
            <p className="text-sm text-muted-foreground">正在跳转到文章...</p>
          </>
        )}
        {status === 'error' && (
          <>
            <XCircle size={32} className="mx-auto mb-4 text-destructive" />
            <p className="text-foreground mb-2">保存失败</p>
            <p className="text-sm text-muted-foreground mb-4">{errorMsg}</p>
            <Button onClick={() => window.location.reload()}>重试</Button>
          </>
        )}
        {status === 'no-url' && (
          <>
            <XCircle size={32} className="mx-auto mb-4 text-muted-foreground" />
            <p className="text-foreground mb-2">未检测到链接</p>
            <p className="text-sm text-muted-foreground mb-4">请从浏览器分享菜单分享一个网页链接</p>
            <Button variant="outline" onClick={() => navigate('/')}>返回首页</Button>
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: LoginPage 支持 redirect 参数**

在 `web/src/pages/LoginPage.tsx` 中：

添加 import：
```tsx
import { useSearchParams } from 'react-router-dom';
```

在组件内添加：
```tsx
const [searchParams] = useSearchParams();
const redirect = searchParams.get('redirect') || '/';
```

将 `navigate('/')` 替换为 `navigate(redirect)`。

- [ ] **Step 4: App.tsx 添加路由**

在 `web/src/App.tsx` 中：

添加 import：
```tsx
import ShareTargetPage from './pages/ShareTargetPage';
```

在 `<Routes>` 内，`<Route path="/login" .../>` 之后添加：
```tsx
<Route path="/share-target" element={<ShareTargetPage />} />
```

注意：此路由在 `<ProtectedRoute>` 之外，ShareTargetPage 自行处理认证。

- [ ] **Step 5: 验证构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -5"`

- [ ] **Step 6: Commit**

```bash
git add web/vite.config.ts web/src/pages/ShareTargetPage.tsx web/src/pages/LoginPage.tsx web/src/App.tsx
git commit -m "feat(mobile): add Web Share Target API for saving links from browser share menu"
```

---

### Task 9: M9 — 通用 useSwipe hook

**Files:**
- Create: `web/src/hooks/useSwipe.ts`
- Create: `web/src/hooks/__tests__/useSwipe.test.ts`

- [ ] **Step 1: 编写 useSwipe hook**

创建 `web/src/hooks/useSwipe.ts`：

```ts
import { useRef, useCallback, useEffect, useState } from 'react';

interface UseSwipeOptions {
  threshold?: number;
  direction?: 'horizontal' | 'vertical' | 'all';
  edgeStart?: number; // px from edge to start gesture (0 = anywhere)
  edgeSide?: 'left' | 'right'; // which edge for edgeStart
}

interface UseSwipeReturn {
  swipeOffset: { x: number; y: number };
  swipingDirection: 'left' | 'right' | 'up' | 'down' | null;
  isSwiping: boolean;
  ref: React.RefObject<HTMLDivElement | null>;
}

type SwipeCallbacks = {
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  onSwipeUp?: () => void;
  onSwipeDown?: () => void;
};

const DIRECTION_LOCK_THRESHOLD = 10;

export function useSwipe(
  callbacks: SwipeCallbacks,
  options: UseSwipeOptions = {},
): UseSwipeReturn {
  const {
    threshold = 80,
    direction = 'horizontal',
    edgeStart = 0,
    edgeSide = 'left',
  } = options;

  const ref = useRef<HTMLDivElement>(null);
  const [swipeOffset, setSwipeOffset] = useState({ x: 0, y: 0 });
  const [swipingDirection, setSwipingDirection] = useState<'left' | 'right' | 'up' | 'down' | null>(null);
  const [isSwiping, setIsSwiping] = useState(false);

  const touchStartRef = useRef({ x: 0, y: 0 });
  const currentOffsetRef = useRef({ x: 0, y: 0 });
  const lockedDirectionRef = useRef<'h' | 'v' | null>(null);
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  const handleTouchStart = useCallback((e: TouchEvent) => {
    const touch = e.touches[0];
    touchStartRef.current = { x: touch.clientX, y: touch.clientY };
    lockedDirectionRef.current = null;

    if (edgeStart > 0) {
      const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const relX = touch.clientX - rect.left;
      if (edgeSide === 'left' && relX > edgeStart) return;
      if (edgeSide === 'right' && relX < rect.width - edgeStart) return;
    }
  }, [edgeStart, edgeSide]);

  const handleTouchMove = useCallback((e: TouchEvent) => {
    const touch = e.touches[0];
    const dx = touch.clientX - touchStartRef.current.x;
    const dy = touch.clientY - touchStartRef.current.y;

    // Lock direction after moving past threshold
    if (!lockedDirectionRef.current) {
      if (Math.abs(dx) > DIRECTION_LOCK_THRESHOLD || Math.abs(dy) > DIRECTION_LOCK_THRESHOLD) {
        if (Math.abs(dx) > Math.abs(dy)) {
          if (direction === 'vertical') return;
          lockedDirectionRef.current = 'h';
        } else {
          if (direction === 'horizontal') return;
          lockedDirectionRef.current = 'v';
        }
        setIsSwiping(true);
      } else {
        return;
      }
    }

    // Prevent default for locked horizontal direction
    if (lockedDirectionRef.current === 'h') {
      e.preventDefault();
    }

    // Update offset based on locked direction
    if (lockedDirectionRef.current === 'h') {
      const offset = { x: dx, y: 0 };
      currentOffsetRef.current = offset;
      setSwipeOffset(offset);
      setSwipingDirection(dx > 0 ? 'right' : 'left');
    } else {
      const offset = { x: 0, y: dy };
      currentOffsetRef.current = offset;
      setSwipeOffset(offset);
      setSwipingDirection(dy > 0 ? 'down' : 'up');
    }
  }, [direction]);

  const handleTouchEnd = useCallback(() => {
    if (!isSwiping && !lockedDirectionRef.current) {
      setSwipeOffset({ x: 0, y: 0 });
      setSwipingDirection(null);
      return;
    }

    // Read from ref to avoid stale closure value
    const { x, y } = currentOffsetRef.current;
    const cbs = callbacksRef.current;

    if (Math.abs(x) > threshold) {
      if (x < 0) cbs.onSwipeLeft?.();
      else cbs.onSwipeRight?.();
    }
    if (Math.abs(y) > threshold) {
      if (y < 0) cbs.onSwipeUp?.();
      else cbs.onSwipeDown?.();
    }

    setSwipeOffset({ x: 0, y: 0 });
    setSwipingDirection(null);
    setIsSwiping(false);
    lockedDirectionRef.current = null;
  }, [isSwiping, threshold]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchmove', handleTouchMove, { passive: false });
    el.addEventListener('touchend', handleTouchEnd, { passive: true });

    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('touchend', handleTouchEnd);
    };
  }, [handleTouchStart, handleTouchMove, handleTouchEnd]);

  return { swipeOffset, swipingDirection, isSwiping, ref };
}
```

- [ ] **Step 2: 编写 useSwipe 单元测试**

创建 `web/src/hooks/__tests__/useSwipe.test.ts`：

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSwipe } from '../useSwipe';

function createTouchEvent(type: string, touches: Touch[]) {
  return new TouchEvent(type, {
    touches,
    changedTouches: touches,
    bubbles: true,
    cancelable: true,
  });
}

describe('useSwipe', () => {
  let container: HTMLDivElement;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
  });

  it('calls onSwipeLeft when swiping left past threshold', async () => {
    const onSwipeLeft = vi.fn();
    const { result } = renderHook(() => useSwipe({ onSwipeLeft }, { threshold: 80 }));

    act(() => {
      Object.defineProperty(result.current.ref, 'current', { value: container, writable: true });
    });

    // Simulate touch sequence
    act(() => {
      container.dispatchEvent(createTouchEvent('touchstart', [new Touch({ identifier: 0, target: container, clientX: 200, clientY: 100 })]));
    });
    act(() => {
      container.dispatchEvent(createTouchEvent('touchmove', [new Touch({ identifier: 0, target: container, clientX: 100, clientY: 100 })]));
    });
    act(() => {
      container.dispatchEvent(createTouchEvent('touchend', [new Touch({ identifier: 0, target: container, clientX: 100, clientY: 100 })]));
    });

    expect(onSwipeLeft).toHaveBeenCalled();
  });

  it('does not trigger below threshold', async () => {
    const onSwipeRight = vi.fn();
    const { result } = renderHook(() => useSwipe({ onSwipeRight }, { threshold: 80 }));

    act(() => {
      Object.defineProperty(result.current.ref, 'current', { value: container, writable: true });
    });

    act(() => {
      container.dispatchEvent(createTouchEvent('touchstart', [new Touch({ identifier: 0, target: container, clientX: 100, clientY: 100 })]));
    });
    act(() => {
      container.dispatchEvent(createTouchEvent('touchmove', [new Touch({ identifier: 0, target: container, clientX: 150, clientY: 100 })]));
    });
    act(() => {
      container.dispatchEvent(createTouchEvent('touchend', [new Touch({ identifier: 0, target: container, clientX: 150, clientY: 100 })]));
    });

    expect(onSwipeRight).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 3: 运行测试**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vitest run src/hooks/__tests__/useSwipe.test.ts 2>&1 | tail -20"`

- [ ] **Step 4: Commit**

```bash
git add web/src/hooks/useSwipe.ts web/src/hooks/__tests__/useSwipe.test.ts
git commit -m "feat(mobile): add useSwipe hook for touch gesture handling"
```

---

### Task 10: M10+M12 — 滑动手势（左滑返回 + 左右切换文章）

> 注意：M10（边缘返回）和 M12（内容区域切换文章）合并为一个任务，使用同一个 useSwipe hook。
> 边缘右滑（左边缘 30px 内起始）→ 返回上一页；内容区域右滑 → 上一篇文章；左滑 → 下一篇文章。

**Files:**
- Modify: `web/src/components/EntryCard.tsx:28`
- Modify: `web/src/pages/EntryListPage.tsx`
- Modify: `web/src/hooks/useKeyboardShortcuts.ts:72`
- Modify: `web/src/pages/EntryDetailPage.tsx`

- [ ] **Step 1: EntryCard 导航时传递列表上下文**

在 `web/src/components/EntryCard.tsx` 中，添加 props：

```tsx
entryIndex?: number;
entryIds?: string[];
```

修改 `<Link>` 添加 `state`：
```tsx
<Link
  to={`/entry/${entry.id}`}
  state={entryIds ? { entryIds, currentIndex: entryIndex } : undefined}
  className="block"
>
```

- [ ] **Step 2: EntryListPage 传递列表上下文给 EntryCard**

在 `web/src/pages/EntryListPage.tsx` 中，修改 EntryCard 调用：

```tsx
<EntryCard
  entry={entry}
  selected={i === selectedIndex || selectedIds.has(entry.id)}
  onDomainClick={(d) => setDomain(d)}
  selectionMode={selectionMode}
  entrySelected={selectedIds.has(entry.id)}
  onToggleSelect={() => toggleSelect(entry.id)}
  entryIndex={i}
  entryIds={entries.map(e => e.id)}
/>
```

- [ ] **Step 3: useKeyboardShortcuts 传递列表上下文**

在 `web/src/hooks/useKeyboardShortcuts.ts` 中，修改 `useListKeyboardNav` 的 `Enter`/`o` 键处理：

```ts
case 'Enter':
case 'o':
  if (currentEntries[currentSelected]) {
    const entryIds = currentEntries.map(e => e.id);
    navigate(`/entry/${currentEntries[currentSelected].id}`, {
      state: { entryIds, currentIndex: currentSelected },
    });
  }
  break;
```

- [ ] **Step 4: EntryDetailPage 集成滑动手势**

在 `web/src/pages/EntryDetailPage.tsx` 中：

添加 import：
```tsx
import { useLocation } from 'react-router-dom';
import { useSwipe } from '../hooks/useSwipe';
```

在组件内添加：
```tsx
const location = useLocation();
const listContext = location.state as { entryIds?: string[]; currentIndex?: number } | null;

const navigateToEntry = (direction: 'prev' | 'next') => {
  if (!listContext?.entryIds || listContext.currentIndex === undefined) return;
  const newIndex = direction === 'prev' ? listContext.currentIndex - 1 : listContext.currentIndex + 1;
  if (newIndex < 0 || newIndex >= listContext.entryIds.length) return;
  const newId = listContext.entryIds[newIndex];
  navigate(`/entry/${newId}`, {
    state: { entryIds: listContext.entryIds, currentIndex: newIndex },
    replace: true,
  });
};

// 边缘右滑返回，内容区域左右切换文章
// useSwipe 的 edgeStart/edgeSide 仅影响 touchstart 是否"接受"手势
// 边缘起始的右滑 → 返回；内容区域右滑 → 上一篇文章
const { swipeOffset, isSwiping: isGestureActive, ref: gestureRef } = useSwipe(
  {
    onSwipeRight: () => {
      // 如果有列表上下文且不是第一篇，切换到上一篇；否则返回
      if (listContext?.entryIds && listContext.currentIndex !== undefined && listContext.currentIndex > 0) {
        navigateToEntry('prev');
      } else {
        navigate(-1);
      }
    },
    onSwipeLeft: () => navigateToEntry('next'),
  },
  { threshold: 100, direction: 'horizontal' },
);
```

将最外层 `<div className="flex gap-0 lg:-mx-4">` 改为：
```tsx
<div
  ref={isMobile ? gestureRef : undefined}
  className="flex gap-0 lg:-mx-4"
  style={isMobile && isGestureActive ? {
    transform: `translateX(${swipeOffset.x}px)`,
    transition: swipeOffset.x === 0 ? 'transform 0.2s ease-out' : 'none',
  } : undefined}
>
```

- [ ] **Step 5: 验证构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -5"`

- [ ] **Step 6: Commit**

```bash
git add web/src/components/EntryCard.tsx web/src/pages/EntryListPage.tsx web/src/hooks/useKeyboardShortcuts.ts web/src/pages/EntryDetailPage.tsx
git commit -m "feat(mobile): add swipe gestures for back navigation and article switching"
```

---

### Task 11: M11 — 下拉刷新

**Files:**
- Modify: `web/src/pages/EntryListPage.tsx`

- [ ] **Step 1: 集成下拉刷新手势**

在 `web/src/pages/EntryListPage.tsx` 中：

添加 import：
```tsx
import { useSwipe } from '../hooks/useSwipe';
import { useIsMobile } from '../hooks/use-mobile';
```

在组件内添加：
```tsx
const isMobile = useIsMobile();
const [isRefreshing, setIsRefreshing] = useState(false);

const handleRefresh = async () => {
  setIsRefreshing(true);
  await qc.invalidateQueries({ queryKey: ['entries-infinite'] });
  setIsRefreshing(false);
};

const { swipeOffset: refreshOffset, isSwiping: isPulling, ref: refreshRef } = useSwipe(
  { onSwipeDown: handleRefresh },
  { threshold: 60, direction: 'vertical' },
);

// 仅在页面滚动到顶部时启用下拉刷新
useEffect(() => {
  if (!isMobile || !refreshRef.current) return;
  const el = refreshRef.current;
  const checkScrollTop = () => {
    el.style.touchAction = window.scrollY === 0 ? 'pan-x' : 'auto';
  };
  window.addEventListener('scroll', checkScrollTop, { passive: true });
  checkScrollTop();
  return () => window.removeEventListener('scroll', checkScrollTop);
}, [isMobile]);
```

在最外层 `<div>` 添加 ref（仅移动端）和刷新指示器：

将 `<div>` (第 213 行) 改为：
```tsx
<div ref={isMobile ? refreshRef : undefined}>
```

在 `<div>` 内部最前面添加刷新指示器：
```tsx
{(isPulling || isRefreshing) && (
  <div className="flex items-center justify-center py-3 text-sm text-muted-foreground">
    <Loader2 size={16} className={`mr-2 ${isRefreshing ? 'animate-spin' : ''}`} />
    {isRefreshing ? '刷新中...' : '下拉刷新'}
  </div>
)}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/pages/EntryListPage.tsx
git commit -m "feat(mobile): add pull-to-refresh gesture on entry list"
```

---

### Task 12: 最终验证

- [ ] **Step 1: 完整构建**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vite build 2>&1 | tail -10"`

- [ ] **Step 2: 运行前端测试**

Run: `docker compose run --rm lettura sh -c "cd /app/web && npx vitest run 2>&1 | tail -20"`

- [ ] **Step 3: 在浏览器中手动验证**

启动服务后，在 DevTools 设备模拟器中验证：
- iPhone 14 Pro: AnnotationsSidebar 底部 Sheet、批量操作栏不遮挡底部导航、Settings 卡片布局、tab 栏滚动
- 下拉刷新、左右滑动切换文章
- Web Share Target: 直接访问 `/share-target?url=https://example.com`

- [ ] **Step 4: 最终 commit（如有修复）**

```bash
git add -A
git commit -m "chore(mobile): fix final build/test issues"
```
