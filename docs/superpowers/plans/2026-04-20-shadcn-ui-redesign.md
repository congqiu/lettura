# shadcn/ui 暖色主题前端重设计 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用 shadcn/ui (New York 风格) + 暖色主题全面重构前端样式和布局，引入响应式侧边栏导航。

**Architecture:** shadcn/ui 组件替代手写 UI 组件，通过 CSS 变量控制暖色主题，桌面端侧边栏 + 移动端底栏导航。保持现有 API/状态管理/路由不变。

**Tech Stack:** React 19, Tailwind CSS v4, shadcn/ui (New York), Radix UI, Sonner, Lucide React

**Design Spec:** `docs/superpowers/specs/2026-04-20-shadcn-ui-redesign-design.md`

---

## File Structure

```
web/src/
├── components/
│   ├── ui/                        # shadcn 自动生成 (Task 1)
│   │   ├── button.tsx
│   │   ├── input.tsx
│   │   ├── badge.tsx
│   │   ├── separator.tsx
│   │   ├── skeleton.tsx
│   │   ├── dialog.tsx
│   │   ├── alert-dialog.tsx
│   │   ├── sheet.tsx
│   │   ├── tabs.tsx
│   │   ├── dropdown-menu.tsx
│   │   ├── sidebar.tsx            # 含 SidebarProvider, SidebarContent 等
│   │   ├── command.tsx
│   │   └── sonner.tsx
│   ├── layout/                    # 新建 (Task 3)
│   │   ├── AppSidebar.tsx         # 桌面端侧边栏
│   │   └── MobileBottomNav.tsx    # 移动端底栏导航
│   ├── AddEntryForm.tsx           # 重构 (Task 5)
│   ├── ContentEditor.tsx          # 重构工具栏 (Task 7)
│   ├── EntryCard.tsx              # 暖色样式 (Task 5)
│   ├── AnnotationsSidebar.tsx     # 暖色样式 (Task 7)
│   ├── ConfirmDialog.tsx          # 替换为 AlertDialog (Task 4)
│   ├── KeyboardShortcutsHelp.tsx  # 替换为 Command (Task 4)
│   ├── Layout.tsx                 # 重构为响应式布局 (Task 3)
│   ├── MobileNav.tsx              # 删除，由 layout/ 替代 (Task 3)
│   ├── ThemeToggle.tsx            # 用 DropdownMenu 重写 (Task 4)
│   ├── Toast.tsx                  # 删除，由 Sonner 替代 (Task 4)
│   ├── NetworkStatus.tsx          # 暖色样式 (Task 5)
│   ├── EmptyState.tsx             # 暖色样式 (Task 5)
│   ├── ErrorState.tsx             # 暖色样式 (Task 5)
│   ├── EntryTags.tsx              # 用 Badge 替代 (Task 7)
│   ├── ErrorBoundary.tsx          # 不变
│   ├── PageCard.tsx               # 暖色样式 (Task 8)
│   ├── PageUploadModal.tsx        # 用 Dialog 重写 (Task 8)
│   ├── PageEditModal.tsx          # 用 Dialog 重写 (Task 8)
│   └── ProtectedRoute.tsx         # 不变
├── lib/
│   └── utils.ts                   # shadcn cn() 函数 (Task 1)
├── pages/
│   ├── EntryListPage.tsx          # 暖色样式 + 搜索框 (Task 5)
│   ├── EntryDetailPage.tsx        # 暖色样式 + 操作按钮 (Task 6)
│   ├── LoginPage.tsx              # 暖色样式 (Task 8)
│   ├── RegisterPage.tsx           # 暖色样式 (Task 8)
│   ├── SettingsPage.tsx           # 暖色样式 (Task 8)
│   ├── MemosPage.tsx              # 暖色样式 (Task 8)
│   └── PagesPage.tsx              # 暖色样式 (Task 8)
├── index.css                      # 重写为暖色 CSS 变量主题 (Task 1)
├── App.tsx                        # 添加 SidebarProvider + Sonner (Task 3)
└── main.tsx                       # 不变
```

---

### Task 1: 安装 shadcn/ui 并配置暖色主题

**Files:**
- Create: `web/components.json`
- Create: `web/src/lib/utils.ts`
- Modify: `web/src/index.css` (完全重写)
- Modify: `web/tsconfig.app.json` (添加路径别名)
- Modify: `web/vite.config.ts` (添加路径别名)
- Modify: `web/index.html` (更新 theme-color)

- [ ] **Step 1: 配置 tsconfig 路径别名**

在 `web/tsconfig.app.json` 的 `compilerOptions` 中添加 `baseUrl` 和 `paths`:

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    },
    ...existing options...
  }
}
```

- [ ] **Step 2: 配置 vite 路径别名**

修改 `web/vite.config.ts`，添加 `resolve.alias`:

```typescript
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3330',
      '/feed': 'http://localhost:3330',
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test-setup.ts',
  },
})
```

- [ ] **Step 3: 创建 shadcn 配置文件**

创建 `web/components.json`:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/index.css",
    "baseColor": "neutral",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  },
  "iconLibrary": "lucide"
}
```

注意: Tailwind v4 要求 `tailwind.config` 留空。

- [ ] **Step 4: 创建 utils.ts**

创建 `web/src/lib/utils.ts`:

```typescript
import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

- [ ] **Step 5: 安装依赖**

Run:
```bash
cd web && npm install clsx tailwind-merge class-variance-authority
```

- [ ] **Step 6: 重写 index.css 为暖色主题**

将 `web/src/index.css` 完全重写为:

```css
@import "tailwindcss";
@plugin "@tailwindcss/typography";
@custom-variant dark (&:where(.dark, .dark *));

@theme inline {
  --color-background: hsl(var(--background));
  --color-foreground: hsl(var(--foreground));
  --color-card: hsl(var(--card));
  --color-card-foreground: hsl(var(--card-foreground));
  --color-popover: hsl(var(--popover));
  --color-popover-foreground: hsl(var(--popover-foreground));
  --color-primary: hsl(var(--primary));
  --color-primary-foreground: hsl(var(--primary-foreground));
  --color-secondary: hsl(var(--secondary));
  --color-secondary-foreground: hsl(var(--secondary-foreground));
  --color-muted: hsl(var(--muted));
  --color-muted-foreground: hsl(var(--muted-foreground));
  --color-accent: hsl(var(--accent));
  --color-accent-foreground: hsl(var(--accent-foreground));
  --color-destructive: hsl(var(--destructive));
  --color-destructive-foreground: hsl(var(--destructive-foreground));
  --color-border: hsl(var(--border));
  --color-input: hsl(var(--input));
  --color-ring: hsl(var(--ring));
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) + 4px);
  --font-sans: "Inter", ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji";
}

:root {
  --background: 30 50% 97%;
  --foreground: 20 60% 10%;
  --card: 0 0% 100%;
  --card-foreground: 20 60% 10%;
  --popover: 0 0% 100%;
  --popover-foreground: 20 60% 10%;
  --primary: 30 90% 45%;
  --primary-foreground: 0 0% 100%;
  --secondary: 40 40% 90%;
  --secondary-foreground: 20 60% 10%;
  --muted: 40 30% 85%;
  --muted-foreground: 30 30% 40%;
  --accent: 40 40% 90%;
  --accent-foreground: 20 60% 10%;
  --destructive: 20 70% 40%;
  --destructive-foreground: 0 0% 100%;
  --border: 30 20% 80%;
  --input: 30 20% 80%;
  --ring: 30 90% 45%;
  --radius: 0.625rem;
}

.dark {
  --background: 20 10% 10%;
  --foreground: 30 50% 97%;
  --card: 10 5% 15%;
  --card-foreground: 30 50% 97%;
  --popover: 10 5% 15%;
  --popover-foreground: 30 50% 97%;
  --primary: 40 90% 55%;
  --primary-foreground: 20 10% 10%;
  --secondary: 10 5% 18%;
  --secondary-foreground: 30 50% 97%;
  --muted: 20 5% 25%;
  --muted-foreground: 30 10% 65%;
  --accent: 10 5% 18%;
  --accent-foreground: 30 50% 97%;
  --destructive: 0 60% 50%;
  --destructive-foreground: 0 0% 100%;
  --border: 20 5% 25%;
  --input: 20 5% 25%;
  --ring: 40 90% 55%;
}

@layer base {
  * {
    @apply border-border;
  }
  body {
    @apply bg-background text-foreground transition-colors duration-300;
  }
}
```

- [ ] **Step 7: 更新 index.html theme-color**

将 `web/index.html` 中 `<meta name="theme-color" content="#2563eb" />` 改为暖色:
```html
<meta name="theme-color" content="#fefcf3" />
```

- [ ] **Step 8: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```
Expected: 构建成功，无 TypeScript 错误

- [ ] **Step 9: Commit**

```bash
git add web/components.json web/src/lib/utils.ts web/src/index.css web/tsconfig.app.json web/vite.config.ts web/index.html web/package.json web/pnpm-lock.yaml
git commit -m "feat(web): install shadcn/ui foundation with warm theme CSS variables"
```

---

### Task 2: 添加 shadcn/ui 基础组件

**Files:**
- Create: `web/src/components/ui/button.tsx`
- Create: `web/src/components/ui/input.tsx`
- Create: `web/src/components/ui/badge.tsx`
- Create: `web/src/components/ui/separator.tsx`
- Create: `web/src/components/ui/skeleton.tsx`

- [ ] **Step 1: 用 shadcn CLI 添加组件**

Run:
```bash
cd web && npx shadcn@latest add button input badge separator skeleton
```

当提示覆盖时选择 yes。CLI 会自动安装必要的 Radix UI 依赖。

- [ ] **Step 2: 验证组件文件已创建**

检查 `web/src/components/ui/` 目录下存在以下文件:
- button.tsx
- input.tsx
- badge.tsx
- separator.tsx
- skeleton.tsx

- [ ] **Step 3: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```
Expected: 构建成功

- [ ] **Step 4: Commit**

```bash
git add web/src/components/ui/ web/package.json
git commit -m "feat(web): add shadcn base components (button, input, badge, separator, skeleton)"
```

---

### Task 3: 重构布局 — 响应式侧边栏 + 移动端底栏

**Files:**
- Create: `web/src/components/ui/sidebar.tsx`
- Create: `web/src/components/layout/AppSidebar.tsx`
- Create: `web/src/components/layout/MobileBottomNav.tsx`
- Modify: `web/src/components/Layout.tsx` (完全重写)
- Delete: `web/src/components/MobileNav.tsx`
- Modify: `web/src/App.tsx` (添加 SidebarProvider)

- [ ] **Step 1: 用 shadcn CLI 添加 Sidebar 组件**

Run:
```bash
cd web && npx shadcn@latest add sidebar
```

- [ ] **Step 2: 创建 AppSidebar 组件**

创建 `web/src/components/layout/AppSidebar.tsx`:

```tsx
import { NavLink, useNavigate } from 'react-router-dom';
import { BookOpen, Archive, Star, StickyNote, Globe, Settings, LogOut, Sun, Moon, Monitor } from 'lucide-react';
import { useAuthStore } from '../../store/auth';
import { useThemeStore } from '../../store/theme';
import { logout as apiLogout } from '../../api/auth';
import {
  Sidebar, SidebarContent, SidebarFooter, SidebarGroup, SidebarGroupContent,
  SidebarGroupLabel, SidebarHeader, SidebarMenu, SidebarMenuButton, SidebarMenuItem,
} from '@/components/ui/sidebar';
import { Separator } from '@/components/ui/separator';
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';

const navItems = [
  { to: '/', label: '未读', icon: BookOpen, end: true },
  { to: '/archived', label: '归档', icon: Archive, end: false },
  { to: '/starred', label: '收藏', icon: Star, end: false },
  { to: '/memos', label: '便签', icon: StickyNote, end: false },
];

const toolItems = [
  { to: '/pages', label: 'Pages', icon: Globe, end: false },
];

export function AppSidebar() {
  const { logout } = useAuthStore();
  const navigate = useNavigate();
  const { theme, setTheme } = useThemeStore();

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch {}
    }
    logout();
    navigate('/login');
  };

  const themeIcon = theme === 'dark' ? Moon : theme === 'light' ? Sun : Monitor;
  const ThemeIcon = themeIcon;

  return (
    <Sidebar>
      <SidebarHeader className="px-4 py-4">
        <span className="font-bold text-lg text-primary select-none">Lettura</span>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <SidebarMenuButton asChild>
                    <NavLink
                      to={item.to}
                      end={item.end}
                      className={({ isActive }) =>
                        isActive ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground'
                      }
                    >
                      <item.icon size={18} />
                      <span>{item.label}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <Separator className="mx-4 w-auto" />

        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {toolItems.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <SidebarMenuButton asChild>
                    <NavLink
                      to={item.to}
                      end={item.end}
                      className={({ isActive }) =>
                        isActive ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground'
                      }
                    >
                      <item.icon size={18} />
                      <span>{item.label}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter>
        <Separator className="mb-2" />
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild>
              <NavLink
                to="/settings"
                className={({ isActive }) =>
                  isActive ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground'
                }
              >
                <Settings size={18} />
                <span>设置</span>
              </NavLink>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground">
                <ThemeIcon size={18} />
                <span>主题</span>
              </DropdownMenuTrigger>
              <DropdownMenuContent side="top" align="start">
                <DropdownMenuItem onClick={() => setTheme('light')}>
                  <Sun size={16} className="mr-2" /> 浅色
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setTheme('dark')}>
                  <Moon size={16} className="mr-2" /> 深色
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => setTheme('system')}>
                  <Monitor size={16} className="mr-2" /> 跟随系统
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={handleLogout} className="text-muted-foreground">
              <LogOut size={18} />
              <span>退出</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
```

注意: 这里提前引用了 Task 4 的 DropdownMenu 组件。如果构建失败，可以先注释掉主题切换部分，在 Task 4 完成后再启用。或者先在 Task 2 额外安装 `npx shadcn@latest add dropdown-menu`。

- [ ] **Step 3: 创建移动端底栏导航**

创建 `web/src/components/layout/MobileBottomNav.tsx`:

```tsx
import { NavLink } from 'react-router-dom';
import { BookOpen, Archive, Star, StickyNote, MoreHorizontal } from 'lucide-react';
import { Sheet, SheetContent, SheetTrigger } from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { Button } from '@/components/ui/button';
import { useAuthStore } from '../../store/auth';
import { useNavigate } from 'react-router-dom';
import { logout as apiLogout } from '../../api/auth';
import { useState } from 'react';

const bottomNavItems = [
  { to: '/', label: '未读', icon: BookOpen, end: true },
  { to: '/archived', label: '归档', icon: Archive, end: false },
  { to: '/starred', label: '收藏', icon: Star, end: false },
  { to: '/memos', label: '便签', icon: StickyNote, end: false },
];

export function MobileBottomNav() {
  const [sheetOpen, setSheetOpen] = useState(false);
  const { logout } = useAuthStore();
  const navigate = useNavigate();

  const handleLogout = async () => {
    const refreshToken = localStorage.getItem('refresh_token');
    if (refreshToken) {
      try { await apiLogout(refreshToken); } catch {}
    }
    logout();
    navigate('/login');
  };

  return (
    <div className="fixed bottom-0 left-0 right-0 z-40 border-t border-border bg-card lg:hidden">
      <div className="flex items-center justify-around px-2 py-1 pb-[env(safe-area-inset-bottom)]">
        {bottomNavItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.end}
            className={({ isActive }) =>
              `flex flex-col items-center gap-0.5 px-3 py-1.5 text-xs transition-colors ${
                isActive ? 'text-primary font-medium' : 'text-muted-foreground'
              }`
            }
          >
            <item.icon size={20} />
            <span>{item.label}</span>
          </NavLink>
        ))}

        <Sheet open={sheetOpen} onOpenChange={setSheetOpen}>
          <SheetTrigger asChild>
            <button className="flex flex-col items-center gap-0.5 px-3 py-1.5 text-xs text-muted-foreground">
              <MoreHorizontal size={20} />
              <span>更多</span>
            </button>
          </SheetTrigger>
          <SheetContent side="bottom" className="rounded-t-2xl">
            <div className="space-y-1 py-2">
              <NavLink
                to="/pages"
                onClick={() => setSheetOpen(false)}
                className="block px-4 py-3 text-sm rounded-lg hover:bg-accent"
              >
                Pages
              </NavLink>
              <NavLink
                to="/settings"
                onClick={() => setSheetOpen(false)}
                className="block px-4 py-3 text-sm rounded-lg hover:bg-accent"
              >
                设置
              </NavLink>
              <Separator className="my-2" />
              <button
                onClick={() => { setSheetOpen(false); handleLogout(); }}
                className="block w-full text-left px-4 py-3 text-sm rounded-lg hover:bg-accent text-destructive"
              >
                退出登录
              </button>
            </div>
          </SheetContent>
        </Sheet>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: 重写 Layout.tsx**

将 `web/src/components/Layout.tsx` 完全重写为:

```tsx
import { Outlet } from 'react-router-dom';
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar';
import { AppSidebar } from './layout/AppSidebar';
import { MobileBottomNav } from './layout/MobileBottomNav';
import ErrorBoundary from './ErrorBoundary';
import NetworkStatus from './NetworkStatus';
import { Toaster } from '@/components/ui/sonner';

export default function Layout() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset>
        <NetworkStatus />
        {/* 移动端顶栏 */}
        <header className="flex h-14 items-center gap-2 border-b border-border bg-card px-4 lg:hidden">
          <span className="font-bold text-lg text-primary select-none">Lettura</span>
        </header>
        <main className="mx-auto max-w-3xl px-4 py-6 pb-24 lg:pb-6">
          <ErrorBoundary level="page">
            <Outlet />
          </ErrorBoundary>
        </main>
      </SidebarInset>
      <MobileBottomNav />
      <Toaster richColors position="bottom-right" />
    </SidebarProvider>
  );
}
```

- [ ] **Step 5: 更新 App.tsx**

将 `web/src/App.tsx` 中的 `Suspense fallback` 改为使用 shadcn 风格:

```tsx
import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import ErrorBoundary from './components/ErrorBoundary';
import LoginPage from './pages/LoginPage';
import RegisterPage from './pages/RegisterPage';
import EntryListPage from './pages/EntryListPage';
import EntryDetailPage from './pages/EntryDetailPage';

const MemosPage = lazy(() => import('./pages/MemosPage'));
const PagesPage = lazy(() => import('./pages/PagesPage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));

const queryClient = new QueryClient();

function App() {
  return (
    <ErrorBoundary level="app">
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <Suspense fallback={
            <div className="flex items-center justify-center min-h-screen">
              <div className="text-muted-foreground">Loading...</div>
            </div>
          }>
            <Routes>
              <Route path="/login" element={<LoginPage />} />
              <Route path="/register" element={<RegisterPage />} />
              <Route
                path="/"
                element={
                  <ProtectedRoute>
                    <Layout />
                  </ProtectedRoute>
                }
              >
                <Route index element={<EntryListPage filter="unread" />} />
                <Route path="archived" element={<EntryListPage filter="archived" />} />
                <Route path="starred" element={<EntryListPage filter="starred" />} />
                <Route path="entry/:id" element={<EntryDetailPage />} />
                <Route path="memos" element={<MemosPage />} />
                <Route path="pages" element={<PagesPage />} />
                <Route path="settings" element={<SettingsPage />} />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Suspense>
        </BrowserRouter>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
```

- [ ] **Step 6: 删除旧组件**

删除不再使用的文件:
- `web/src/components/MobileNav.tsx`
- `web/src/components/ThemeToggle.tsx`
- `web/src/components/Toast.tsx`

注意: Toast 的 `toast()` 函数导入需要全部替换为 `sonner` 的。先用 grep 确认所有导入 Toast 的文件:
```bash
grep -r "from.*Toast" web/src/ --include="*.tsx" -l
grep -r "from.*Toast" web/src/ --include="*.ts" -l
```

所有 `import { toast } from '../components/Toast'` 替换为 `import { toast } from 'sonner'`。

- [ ] **Step 7: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```
Expected: 构建成功

- [ ] **Step 8: Commit**

```bash
git add -A web/src/
git commit -m "feat(web): add responsive sidebar layout with mobile bottom nav"
```

---

### Task 4: 添加交互组件 (Dialog, Sheet, DropdownMenu, Sonner, Tabs, Command)

**Files:**
- Create: `web/src/components/ui/dialog.tsx`
- Create: `web/src/components/ui/alert-dialog.tsx`
- Create: `web/src/components/ui/sheet.tsx`
- Create: `web/src/components/ui/dropdown-menu.tsx`
- Create: `web/src/components/ui/tabs.tsx`
- Create: `web/src/components/ui/command.tsx`
- Create: `web/src/components/ui/sonner.tsx`
- Modify: `web/src/components/ConfirmDialog.tsx` (用 AlertDialog 重写)
- Modify: `web/src/components/KeyboardShortcutsHelp.tsx` (用 Command + Dialog 重写)

- [ ] **Step 1: 用 shadcn CLI 添加组件**

Run:
```bash
cd web && npx shadcn@latest add dialog alert-dialog sheet dropdown-menu tabs command sonner tooltip
```

- [ ] **Step 2: 用 AlertDialog 重写 ConfirmDialog**

将 `web/src/components/ConfirmDialog.tsx` 重写为:

```tsx
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { AlertTriangle } from 'lucide-react';

interface Props {
  open: boolean;
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: 'danger' | 'default';
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog({
  open, title, message, confirmText = '确定', cancelText = '取消',
  variant = 'default', onConfirm, onCancel,
}: Props) {
  return (
    <AlertDialog open={open} onOpenChange={(v) => { if (!v) onCancel(); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          {title && <AlertDialogTitle>{title}</AlertDialogTitle>}
          <AlertDialogDescription className="flex items-start gap-3">
            {variant === 'danger' && (
              <div className="shrink-0 w-10 h-10 rounded-full bg-destructive/10 flex items-center justify-center">
                <AlertTriangle size={20} className="text-destructive" />
              </div>
            )}
            <span>{message}</span>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{cancelText}</AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className={variant === 'danger' ? 'bg-destructive text-destructive-foreground hover:bg-destructive/90' : ''}
          >
            {confirmText}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

- [ ] **Step 3: 用 Command + Dialog 重写 KeyboardShortcutsHelp**

将 `web/src/components/KeyboardShortcutsHelp.tsx` 重写为:

```tsx
import { useEffect } from 'react';
import {
  Dialog, DialogContent, DialogHeader, DialogTitle,
} from '@/components/ui/dialog';

interface Props {
  open: boolean;
  onClose: () => void;
}

const SECTIONS = [
  {
    title: '导航',
    shortcuts: [
      { keys: ['1'], desc: '未读列表' },
      { keys: ['2'], desc: '归档列表' },
      { keys: ['3'], desc: '收藏列表' },
      { keys: ['4'], desc: '收集箱' },
      { keys: ['h', '←'], desc: '返回上页' },
    ],
  },
  {
    title: '列表',
    shortcuts: [
      { keys: ['j'], desc: '下一篇' },
      { keys: ['k'], desc: '上一篇' },
      { keys: ['Enter', 'o'], desc: '打开文章' },
    ],
  },
  {
    title: '文章',
    shortcuts: [
      { keys: ['s'], desc: '收藏 / 取消收藏' },
      { keys: ['a'], desc: '归档 / 取消归档' },
      { keys: ['e'], desc: '编辑内容' },
    ],
  },
  {
    title: '全局',
    shortcuts: [
      { keys: ['?'], desc: '显示快捷键帮助' },
    ],
  },
];

export default function KeyboardShortcutsHelp({ open, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [open, onClose]);

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-md max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>键盘快捷键</DialogTitle>
        </DialogHeader>
        <div className="space-y-5">
          {SECTIONS.map((section) => (
            <div key={section.title}>
              <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">
                {section.title}
              </h3>
              <div className="space-y-1.5">
                {section.shortcuts.map((shortcut) => (
                  <div key={shortcut.desc} className="flex items-center justify-between">
                    <span className="text-sm">{shortcut.desc}</span>
                    <div className="flex gap-1">
                      {shortcut.keys.map((key) => (
                        <kbd
                          key={key}
                          className="inline-flex items-center justify-center min-w-[24px] h-6 px-1.5 text-xs font-mono bg-secondary border border-border rounded"
                        >
                          {key}
                        </kbd>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 4: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```

- [ ] **Step 5: Commit**

```bash
git add web/src/components/ui/ web/src/components/ConfirmDialog.tsx web/src/components/KeyboardShortcutsHelp.tsx
git commit -m "feat(web): add interactive components and rewrite ConfirmDialog, KeyboardShortcutsHelp"
```

---

### Task 5: 重构列表页组件 (EntryCard, AddEntryForm, EntryListPage)

**Files:**
- Modify: `web/src/components/EntryCard.tsx`
- Modify: `web/src/components/AddEntryForm.tsx`
- Modify: `web/src/pages/EntryListPage.tsx`
- Modify: `web/src/components/EmptyState.tsx`
- Modify: `web/src/components/ErrorState.tsx`
- Modify: `web/src/components/NetworkStatus.tsx`

- [ ] **Step 1: 重写 EntryCard**

将 `web/src/components/EntryCard.tsx` 重写为暖色样式:

```tsx
import { Link } from 'react-router-dom';
import { Star, Archive, ExternalLink, Clock } from 'lucide-react';
import { type EntrySummary } from '../api/entries';
import { timeAgo } from '../utils/time';
import { useEntryActions } from '../hooks/useEntryActions';
import { Button } from '@/components/ui/button';

export default function EntryCard({
  entry,
  selected = false,
  onDomainClick,
}: {
  entry: EntrySummary;
  selected?: boolean;
  onDomainClick?: (domain: string) => void;
}) {
  const { toggleStar, toggleArchive } = useEntryActions(entry.id, entry);

  return (
    <div className={`group bg-card border border-border rounded-xl p-4 transition-all duration-200 hover:shadow-md ${
      selected ? 'ring-2 ring-primary shadow-md' : ''
    }`}>
      <div className="flex flex-col sm:flex-row items-start justify-between gap-3">
        <div className="flex-1 min-w-0 flex flex-col gap-1.5">
          <Link to={`/entry/${entry.id}`} className="block">
            <h3 className="text-base font-semibold text-card-foreground leading-snug line-clamp-2 hover:text-primary transition-colors">
              {entry.title || entry.url}
            </h3>
          </Link>

          <div className="flex items-center gap-2 text-xs text-muted-foreground flex-wrap">
            {entry.domain_name && (
              <button
                onClick={() => onDomainClick?.(entry.domain_name!)}
                className="font-medium hover:text-foreground transition-colors"
                title={`查看 ${entry.domain_name} 的所有文章`}
              >
                {entry.domain_name}
              </button>
            )}
            <span className="w-0.5 h-0.5 rounded-full bg-muted-foreground/40"></span>
            <span>{timeAgo(entry.created_at)}</span>

            {entry.reading_time && (
              <>
                <span className="w-0.5 h-0.5 rounded-full bg-muted-foreground/40"></span>
                <span className="flex items-center gap-1">
                  <Clock size={12} />
                  {entry.reading_time} 分钟
                </span>
              </>
            )}

            {entry.extract_method === 'pending' && (
              <>
                <span className="w-0.5 h-0.5 rounded-full bg-muted-foreground/40"></span>
                <span className="text-amber-600 dark:text-amber-400 font-medium animate-pulse">抓取中...</span>
              </>
            )}
            {entry.extract_method === 'failed' && (
              <>
                <span className="w-0.5 h-0.5 rounded-full bg-muted-foreground/40"></span>
                <span className="text-destructive font-medium">提取失败</span>
              </>
            )}
          </div>

          <div className="flex items-center gap-1 mt-1">
            <Button
              variant="ghost"
              size="icon"
              className={`h-7 w-7 ${entry.is_starred ? 'text-yellow-500' : 'text-muted-foreground'}`}
              onClick={() => toggleStar.mutate()}
              title={entry.is_starred ? '取消收藏' : '收藏'}
            >
              <Star size={16} className={entry.is_starred ? 'fill-current' : ''} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className={`h-7 w-7 ${entry.is_archived ? 'text-green-600' : 'text-muted-foreground'}`}
              onClick={() => toggleArchive.mutate()}
              title={entry.is_archived ? '取消归档' : '归档'}
            >
              <Archive size={16} className={entry.is_archived ? 'fill-current' : ''} />
            </Button>
          </div>
        </div>

        <div className="flex sm:flex-col items-center sm:items-end gap-3 w-full sm:w-auto">
          {entry.preview_picture && (
            <div className="w-20 h-14 sm:w-24 sm:h-16 shrink-0 rounded-lg overflow-hidden border border-border">
              <img src={entry.preview_picture} alt="" className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105" />
            </div>
          )}
          <a
            href={entry.url}
            target="_blank"
            rel="noopener noreferrer"
            className="ml-auto sm:ml-0 flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-primary transition-colors"
            title="访问原始网页"
          >
            原文 <ExternalLink size={10} />
          </a>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 重写 AddEntryForm**

将 `web/src/components/AddEntryForm.tsx` 重写为:

```tsx
import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createEntry } from '../api/entries';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { toast } from 'sonner';

export default function AddEntryForm() {
  const [url, setUrl] = useState('');
  const qc = useQueryClient();

  const mutation = useMutation({
    mutationFn: (url: string) => createEntry(url),
    onSuccess: () => {
      setUrl('');
      qc.invalidateQueries({ queryKey: ['entries'] });
      toast.success('文章已保存');
    },
    onError: (err: any) => {
      toast.error(err.response?.data?.message || '保存失败');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;
    mutation.mutate(url.trim());
  };

  return (
    <form onSubmit={handleSubmit} className="mb-6">
      <div className="flex gap-2">
        <Input
          type="url"
          placeholder="粘贴 URL 保存文章..."
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          className="flex-1 h-10 bg-card"
          required
        />
        <Button type="submit" disabled={mutation.isPending}>
          {mutation.isPending ? '保存中...' : '保存'}
        </Button>
      </div>
    </form>
  );
}
```

- [ ] **Step 3: 重写 EntryListPage**

将 `web/src/pages/EntryListPage.tsx` 重写为:

```tsx
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listEntries, type ListParams } from '../api/entries';
import EntryCard from '../components/EntryCard';
import AddEntryForm from '../components/AddEntryForm';
import ErrorState from '../components/ErrorState';
import EmptyState from '../components/EmptyState';
import { useListKeyboardNav } from '../hooks/useKeyboardShortcuts';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Search } from 'lucide-react';

interface Props {
  filter?: 'unread' | 'archived' | 'starred';
}

const TITLES = { unread: '未读', archived: '归档', starred: '收藏' };

export default function EntryListPage({ filter }: Props) {
  const [search, setSearch] = useState('');
  const [domain, setDomain] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const params: ListParams = {};
  if (filter === 'archived') params.is_archived = true;
  if (filter === 'starred') params.is_starred = true;
  if (filter === 'unread') params.is_archived = false;
  if (search) params.search = search;
  if (domain) params.domain = domain;

  const { data: entries = [], isLoading, error, refetch } = useQuery({
    queryKey: ['entries', filter, search, domain],
    queryFn: () => listEntries(params),
  });

  useListKeyboardNav(entries, selectedIndex, setSelectedIndex);

  const title = TITLES[filter || 'unread'];

  return (
    <div>
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between mb-6 gap-3">
        <div className="flex items-center gap-3">
          <h2 className="text-xl font-bold tracking-tight">{title}</h2>
          {domain && (
            <Badge variant="secondary" className="cursor-pointer" onClick={() => setDomain('')}>
              {domain} &times;
            </Badge>
          )}
        </div>
        <div className="relative w-full sm:w-64">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="text"
            placeholder="搜索..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-9 h-9 bg-card"
          />
        </div>
      </div>

      {!filter || filter === 'unread' ? <AddEntryForm /> : null}

      {isLoading ? (
        <div className="space-y-4">
          {[1, 2, 3].map((i) => (
            <div key={i} className="bg-card border border-border rounded-xl p-4">
              <div className="h-5 w-3/4 bg-muted rounded animate-pulse mb-2" />
              <div className="h-3 w-1/2 bg-muted rounded animate-pulse" />
            </div>
          ))}
        </div>
      ) : error ? (
        <ErrorState onRetry={() => refetch()} />
      ) : entries.length === 0 ? (
        <EmptyState icon="book" title="暂无文章" description="粘贴 URL 保存你的第一篇文章" />
      ) : (
        <div className="space-y-3">
          {entries.map((entry, i) => (
            <EntryCard
              key={entry.id}
              entry={entry}
              selected={i === selectedIndex}
              onDomainClick={(d) => setDomain(d)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: 重写 EmptyState 和 ErrorState**

`web/src/components/EmptyState.tsx`:
```tsx
import { BookOpen, Inbox, Star } from 'lucide-react';
import { Button } from '@/components/ui/button';

const iconMap: Record<string, React.ComponentType<{ size?: number; className?: string }>> = {
  book: BookOpen,
  inbox: Inbox,
  star: Star,
};

interface Props {
  icon?: string;
  title: string;
  description?: string;
  action?: { label: string; onClick: () => void };
}

export default function EmptyState({ icon = 'inbox', title, description, action }: Props) {
  const Icon = iconMap[icon] || Inbox;
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="w-12 h-12 rounded-full bg-secondary flex items-center justify-center mb-4">
        <Icon size={24} className="text-muted-foreground" />
      </div>
      <h3 className="font-semibold text-lg mb-1">{title}</h3>
      {description && <p className="text-sm text-muted-foreground mb-4">{description}</p>}
      {action && <Button variant="outline" onClick={action.onClick}>{action.label}</Button>}
    </div>
  );
}
```

`web/src/components/ErrorState.tsx`:
```tsx
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface Props {
  message?: string;
  onRetry?: () => void;
}

export default function ErrorState({ message = '加载失败', onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <div className="w-12 h-12 rounded-full bg-destructive/10 flex items-center justify-center mb-4">
        <AlertTriangle size={24} className="text-destructive" />
      </div>
      <p className="text-sm text-muted-foreground mb-4">{message}</p>
      {onRetry && <Button variant="outline" size="sm" onClick={onRetry}>重试</Button>}
    </div>
  );
}
```

- [ ] **Step 5: 更新 NetworkStatus 暖色样式**

`web/src/components/NetworkStatus.tsx`:
```tsx
import { useState, useEffect } from 'react';
import { Wifi, WifiOff } from 'lucide-react';

export default function NetworkStatus() {
  const [isOnline, setIsOnline] = useState(navigator.onLine);
  const [showRecovered, setShowRecovered] = useState(false);

  useEffect(() => {
    const handleOnline = () => {
      setIsOnline(true);
      setShowRecovered(true);
      setTimeout(() => setShowRecovered(false), 2000);
    };
    const handleOffline = () => {
      setIsOnline(false);
      setShowRecovered(false);
    };
    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);
    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  if (isOnline && !showRecovered) return null;

  return (
    <div className={`fixed top-0 left-0 right-0 z-[60] flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-medium transition-all ${
      isOnline ? 'bg-primary text-primary-foreground' : 'bg-destructive text-destructive-foreground'
    }`}>
      {isOnline ? <Wifi size={16} /> : <WifiOff size={16} />}
      {isOnline ? '网络已恢复' : '网络连接已断开，请检查网络后重试'}
    </div>
  );
}
```

- [ ] **Step 6: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```

- [ ] **Step 7: Commit**

```bash
git add web/src/components/EntryCard.tsx web/src/components/AddEntryForm.tsx web/src/components/EmptyState.tsx web/src/components/ErrorState.tsx web/src/components/NetworkStatus.tsx web/src/pages/EntryListPage.tsx
git commit -m "feat(web): restyle list page components with warm theme"
```

---

### Task 6: 重构文章详情页 (EntryDetailPage)

**Files:**
- Modify: `web/src/pages/EntryDetailPage.tsx`

- [ ] **Step 1: 重写 EntryDetailPage**

将 `web/src/pages/EntryDetailPage.tsx` 重写为暖色主题 + shadcn 组件:

```tsx
import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import DOMPurify from 'dompurify';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEntry, updateEntry, deleteEntry, refetchEntry } from '../api/entries';
import ContentEditor from '../components/ContentEditor';
import AnnotationsSidebar from '../components/AnnotationsSidebar';
import ErrorState from '../components/ErrorState';
import EntryTags from '../components/EntryTags';
import { useEntryActions } from '../hooks/useEntryActions';
import { getErrorMessage } from '../utils/error';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { Skeleton } from '@/components/ui/skeleton';
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ArrowLeft, Star, Archive, RefreshCw, Edit3, MessageSquare, Trash2, MoreHorizontal } from 'lucide-react';

export default function EntryDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [editing, setEditing] = useState(false);
  const [showAnnotations, setShowAnnotations] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState('');

  const { data: entry, isLoading, error, refetch: refetchEntryQuery } = useQuery({
    queryKey: ['entry', id],
    queryFn: () => getEntry(id!),
    enabled: !!id,
  });

  const invalidate = () => { qc.invalidateQueries({ queryKey: ['entry', id] }); qc.invalidateQueries({ queryKey: ['entries'] }); };

  const { toggleStar, toggleArchive } = useEntryActions(
    id!,
    { is_starred: entry?.is_starred ?? false, is_archived: entry?.is_archived ?? false },
  );
  const saveContent = useMutation({
    mutationFn: (html: string) => updateEntry(id!, { content: html }),
    onSuccess: () => { setEditing(false); invalidate(); },
    onError: () => toast.error('保存内容失败'),
  });
  const saveTitle = useMutation({
    mutationFn: (title: string) => updateEntry(id!, { title }),
    onSuccess: () => { setEditingTitle(false); invalidate(); },
    onError: () => toast.error('保存标题失败'),
  });
  const remove = useMutation({
    mutationFn: () => deleteEntry(id!),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['entries'] }); navigate('/'); },
    onError: () => toast.error('删除失败'),
  });
  const refetch = useMutation({
    mutationFn: () => refetchEntry(id!),
    onSuccess: () => { invalidate(); toast.success('已重新抓取'); },
    onError: (err: unknown) => { toast.error(getErrorMessage(err, '重新抓取失败')); },
  });

  if (isLoading) return (
    <div className="space-y-4 py-8">
      <Skeleton className="h-8 w-3/4" />
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-64 w-full" />
    </div>
  );
  if (error) return <div className="py-8"><ErrorState message="文章加载失败" onRetry={() => refetchEntryQuery()} /></div>;
  if (!entry) return <div className="py-8"><ErrorState message="文章未找到" /></div>;

  return (
    <div className="flex gap-0 -mx-4">
      <div className={`flex-1 px-4 ${showAnnotations ? 'max-w-3xl' : 'max-w-3xl mx-auto'}`}>
        <Button variant="ghost" size="sm" onClick={() => navigate(-1)} className="mb-4 -ml-2 text-muted-foreground">
          <ArrowLeft size={16} className="mr-1" /> 返回
        </Button>

        {editingTitle ? (
          <div className="flex items-center gap-2 mb-2">
            <input
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              className="flex-1 text-2xl font-bold px-2 py-1 border border-input rounded-lg bg-card text-card-foreground focus:outline-none focus:ring-2 focus:ring-ring"
              autoFocus
              onKeyDown={(e) => { if (e.key === 'Enter') saveTitle.mutate(titleDraft); if (e.key === 'Escape') setEditingTitle(false); }}
            />
            <Button size="sm" onClick={() => saveTitle.mutate(titleDraft)}>保存</Button>
            <Button size="sm" variant="ghost" onClick={() => setEditingTitle(false)}>取消</Button>
          </div>
        ) : (
          <h1
            className="text-2xl font-bold mb-2 cursor-pointer hover:text-primary group"
            onClick={() => { setTitleDraft(entry.title || ''); setEditingTitle(true); }}
            title="点击编辑标题"
          >
            {entry.title || '无标题'}
            <span className="text-sm font-normal text-muted-foreground ml-2 opacity-0 group-hover:opacity-100 transition-opacity">编辑</span>
          </h1>
        )}

        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-4">
          {entry.domain_name && (
            <a href={entry.url} target="_blank" rel="noopener noreferrer" className="hover:text-foreground hover:underline">
              {entry.domain_name}
            </a>
          )}
          {entry.published_by && <span>作者: {entry.published_by}</span>}
          {entry.reading_time && <span>{entry.reading_time} 分钟阅读</span>}
          {entry.language && <span>{entry.language}</span>}
        </div>

        <div className="flex gap-2 mb-6 flex-wrap">
          <Button
            variant={entry.is_starred ? 'default' : 'outline'}
            size="sm"
            onClick={() => toggleStar.mutate()}
            className={entry.is_starred ? 'bg-yellow-500 hover:bg-yellow-600 text-white' : ''}
          >
            <Star size={14} className={`mr-1 ${entry.is_starred ? 'fill-current' : ''}`} />
            {entry.is_starred ? '已收藏' : '收藏'}
          </Button>
          <Button
            variant={entry.is_archived ? 'default' : 'outline'}
            size="sm"
            onClick={() => toggleArchive.mutate()}
            className={entry.is_archived ? 'bg-green-600 hover:bg-green-700 text-white' : ''}
          >
            <Archive size={14} className={`mr-1 ${entry.is_archived ? 'fill-current' : ''}`} />
            {entry.is_archived ? '已归档' : '归档'}
          </Button>

          <Button variant="outline" size="sm" onClick={() => refetch.mutate()} disabled={refetch.isPending}>
            <RefreshCw size={14} className={`mr-1 ${refetch.isPending ? 'animate-spin' : ''}`} />
            {refetch.isPending ? '抓取中...' : '重新抓取'}
          </Button>

          {entry.content && !editing && (
            <Button variant="outline" size="sm" onClick={() => setEditing(true)}>
              <Edit3 size={14} className="mr-1" /> 编辑内容
            </Button>
          )}

          <Button
            variant={showAnnotations ? 'default' : 'outline'}
            size="sm"
            onClick={() => setShowAnnotations(!showAnnotations)}
          >
            <MessageSquare size={14} className="mr-1" />
            批注
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8">
                <MoreHorizontal size={16} />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem
                className="text-destructive focus:text-destructive"
                onClick={() => { if (confirm('确定删除这篇文章？')) remove.mutate(); }}
              >
                <Trash2 size={14} className="mr-2" /> 删除
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {editing && entry.content ? (
          <ContentEditor content={entry.content} onSave={(html) => saveContent.mutate(html)} onCancel={() => setEditing(false)} />
        ) : entry.extract_method === 'pending' ? (
          <p className="text-yellow-600 dark:text-yellow-400">正在抓取内容...</p>
        ) : entry.extract_method === 'failed' ? (
          <p className="text-destructive">内容提取失败。
            <a href={entry.url} target="_blank" className="underline ml-1">查看原文</a>
          </p>
        ) : entry.content ? (
          <article className="prose prose-gray dark:prose-invert max-w-none" dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(entry.content) }} />
        ) : (
          <p className="text-muted-foreground">暂无内容</p>
        )}

        <Separator className="my-6" />

        <div>
          <a href={entry.url} target="_blank" rel="noopener noreferrer"
            className="text-sm text-primary hover:underline">
            查看原文 ↗ {entry.domain_name && `(${entry.domain_name})`}
          </a>
        </div>

        {id && <EntryTags entryId={id} />}
      </div>

      {showAnnotations && id && <AnnotationsSidebar entryId={id} />}
    </div>
  );
}
```

- [ ] **Step 2: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```

- [ ] **Step 3: Commit**

```bash
git add web/src/pages/EntryDetailPage.tsx
git commit -m "feat(web): restyle entry detail page with warm theme and shadcn components"
```

---

### Task 7: 重构编辑器、标签、批注组件

**Files:**
- Modify: `web/src/components/ContentEditor.tsx`
- Modify: `web/src/components/EntryTags.tsx`
- Modify: `web/src/components/AnnotationsSidebar.tsx`

- [ ] **Step 1: 重写 ContentEditor 工具栏**

读取当前 `web/src/components/ContentEditor.tsx` 内容，将所有工具栏按钮替换为 shadcn Button:

```tsx
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
// ...existing imports...

// 替换所有 <button> 为 <Button variant="ghost" size="icon">
// 工具栏的 <div className="flex gap-1 ..."> 中的按钮改为:
// <Button variant="ghost" size="icon" className="h-8 w-8" onClick={...} title="...">
//   <Icon size={16} />
// </Button>
// 工具栏分组用 <Separator orientation="vertical" className="h-6 mx-1" /> 分隔
```

具体实现时读取当前文件内容，保持 Tiptap 逻辑不变，只替换工具栏按钮和样式。

- [ ] **Step 2: 重写 EntryTags 用 Badge**

将 `web/src/components/EntryTags.tsx` 中的标签展示替换为 shadcn Badge:

```tsx
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
// 标签展示: <Badge variant="secondary">{tag.name} <button onClick={...}>&times;</button></Badge>
// 添加标签: <Input ... /> + <Button size="sm">添加</Button>
```

- [ ] **Step 3: 更新 AnnotationsSidebar 暖色样式**

读取当前 `web/src/components/AnnotationsSidebar.tsx`，将硬编码的 gray/blue 色值替换为语义色:
- `bg-white dark:bg-gray-900` → `bg-card`
- `text-gray-900 dark:text-gray-100` → `text-card-foreground`
- `text-gray-500` → `text-muted-foreground`
- `border-gray-200` → `border-border`
- `bg-blue-50` → `bg-secondary`
- 按钮换为 shadcn Button

- [ ] **Step 4: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```

- [ ] **Step 5: Commit**

```bash
git add web/src/components/ContentEditor.tsx web/src/components/EntryTags.tsx web/src/components/AnnotationsSidebar.tsx
git commit -m "feat(web): restyle editor, tags, and annotations with shadcn components"
```

---

### Task 8: 重构其余页面 (Login, Register, Settings, Memos, Pages)

**Files:**
- Modify: `web/src/pages/LoginPage.tsx`
- Modify: `web/src/pages/RegisterPage.tsx`
- Modify: `web/src/pages/SettingsPage.tsx`
- Modify: `web/src/pages/MemosPage.tsx`
- Modify: `web/src/pages/PagesPage.tsx`
- Modify: `web/src/components/PageCard.tsx`
- Modify: `web/src/components/PageUploadModal.tsx`
- Modify: `web/src/components/PageEditModal.tsx`

- [ ] **Step 1: 重写 LoginPage**

读取当前文件，将样式替换为暖色主题:
- 外层: `bg-gray-50 dark:bg-gray-950` → `bg-background`
- 卡片: `bg-white dark:bg-gray-800` → `bg-card border border-border rounded-xl shadow-sm`
- 输入框: 替换为 `<Input />`
- 按钮: 替换为 `<Button className="w-full">登录</Button>`
- 错误信息: `text-red-500` → `text-destructive`
- 链接: `text-blue-600` → `text-primary`

- [ ] **Step 2: 重写 RegisterPage**

同 LoginPage 的替换规则。

- [ ] **Step 3: 重写 SettingsPage**

读取当前文件，将:
- 所有 `<button>` 替换为 `<Button variant="outline">` 或 `<Button variant="destructive">`
- 所有 `<input>` 替换为 `<Input />`
- 硬编码颜色替换为语义色

- [ ] **Step 4: 重写 MemosPage**

读取当前文件，暖色样式替换:
- 卡片: `bg-white dark:bg-gray-900 border-gray-100 dark:border-gray-800` → `bg-card border border-border`
- 按钮: 替换为 shadcn Button
- 颜色: 灰/蓝 → 语义色

- [ ] **Step 5: 重写 PagesPage + PageCard**

PagesPage: 同上规则。
PageCard: 同 EntryCard 的暖色替换规则，按钮换 shadcn Button。

- [ ] **Step 6: 重写 PageUploadModal 和 PageEditModal**

用 shadcn Dialog 替代当前手写的 modal:
```tsx
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
// 保持逻辑不变，UI 换为 shadcn Dialog
```

- [ ] **Step 7: 验证构建**

Run:
```bash
cd web && npx tsc -b && npx vite build
```

- [ ] **Step 8: Commit**

```bash
git add web/src/pages/ web/src/components/PageCard.tsx web/src/components/PageUploadModal.tsx web/src/components/PageEditModal.tsx
git commit -m "feat(web): restyle remaining pages with warm theme and shadcn components"
```

---

### Task 9: 最终验证和清理

**Files:**
- 全局检查

- [ ] **Step 1: 检查遗留的硬编码颜色**

Run:
```bash
grep -rn "bg-gray-\|text-gray-\|border-gray-\|bg-blue-\|text-blue-\|border-blue-" web/src/ --include="*.tsx" | grep -v "node_modules" | grep -v "prose"
```

将找到的所有残留硬编码颜色替换为语义色。prose 相关的可保留。

- [ ] **Step 2: 检查遗留的旧组件导入**

Run:
```bash
grep -rn "from.*MobileNav\|from.*Toast\|from.*ThemeToggle" web/src/ --include="*.tsx"
```

确保没有残留的旧组件导入。

- [ ] **Step 3: 完整构建验证**

Run:
```bash
cd web && npx tsc -b && npx vite build
```
Expected: 零错误

- [ ] **Step 4: 运行测试**

Run:
```bash
cd web && npm test
```

确保所有现有测试仍然通过（可能需要更新 import 路径）。

- [ ] **Step 5: 启动 Docker 环境进行视觉验证**

Run:
```bash
cd /home/cc/workspace/lettura && ./dev.sh build
```

在浏览器中访问 http://localhost:3330 验证:
- 浅色模式暖色主题
- 暗色模式暖色主题
- 桌面端侧边栏导航
- 移动端底栏导航
- 列表页卡片样式
- 文章详情页
- 所有弹窗/对话框

- [ ] **Step 6: Commit**

```bash
git add -A web/src/
git commit -m "chore(web): final cleanup and color migration to semantic tokens"
```
