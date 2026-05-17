# Lettura UI/UX 优化路线图

> 基于代码审阅与交互/视觉设计原则，制定的多阶段优化方案。
> 优先级：P0（立即执行）→ P1（近期）→ P2（中期）→ P3（长期）

---

## 阶段一：快速修复（Quick Wins）

**目标**：低风险、低工时、高用户感知度的细节修复。预期 1–2 天完成。

### 1.1 删除确认统一为应用内 Dialog（P0）

**问题**：`EntryDetailPage.tsx:383` 使用原生 `confirm()`，与精致的设计系统严重割裂。

**方案**：复用 `EntryListPage` 已引入的 `AlertDialog`，为单篇文章删除增加统一确认流程。

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`

**关键改动**：
```tsx
// 新增状态
const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);

// 替换原生 confirm
<DropdownMenuItem
  className="text-destructive focus:text-destructive rounded-lg cursor-pointer"
  onClick={() => setDeleteConfirmOpen(true)}
>
  <Trash2 size={14} className="mr-2" /> 删除
</DropdownMenuItem>

// 新增 AlertDialog（同 EntryListPage 风格）
<AlertDialog open={deleteConfirmOpen} onOpenChange={setDeleteConfirmOpen}>
  <AlertDialogContent className="rounded-2xl">
    <AlertDialogHeader>
      <AlertDialogTitle>确认删除</AlertDialogTitle>
      <AlertDialogDescription>
        确定要删除这篇文章吗？此操作不可撤销。
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
      <AlertDialogAction
        variant="destructive"
        className="rounded-lg"
        onClick={() => { remove.mutate(); setDeleteConfirmOpen(false); }}
      >
        删除
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

---

### 1.2 操作成功反馈补全（P0）

**问题**：收藏/归档等操作仅在失败时 `toast.error()`，成功时无反馈，用户不确定操作是否生效。

**方案**：在 `useEntryActions` hook 中为 toggle 操作增加轻量成功 Toast。

**涉及文件**：
- `web/src/hooks/useEntryActions.ts`

**关键改动**：
```ts
// toggleStar.onSuccess 中增加
toa st.success(entry.is_starred ? '已取消收藏' : '已收藏');

// toggleArchive.onSuccess 中增加
toast.success(entry.is_archived ? '已取消归档' : '已归档');
```

> 注：Toast 文案采用"已XX"而非"操作成功"，更直接、更短。

---

### 1.3 划词工具栏增加箭头指向（P0）

**问题**：浮动工具栏是矩形，用户难以直观建立"这个工具栏属于我选中的文字"的心理模型。

**方案**：在 `.selection-toolbar` 底部中央增加一个向下的 CSS 三角形，指向选区中心。

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`（工具栏 style 部分）
- `web/src/index.css`（新增工具栏箭头样式）

**关键改动**：
```css
/* index.css */
.selection-toolbar::after {
  content: '';
  position: absolute;
  bottom: -6px;
  left: 50%;
  transform: translateX(-50%);
  width: 0;
  height: 0;
  border-left: 6px solid transparent;
  border-right: 6px solid transparent;
  border-top: 6px solid hsl(var(--card));
  filter: drop-shadow(0 1px 1px rgba(0,0,0,0.05));
}
```

同时调整工具栏 `top` 计算，将 `rect.top - 8` 改为 `rect.top - 14`，为箭头留出空间。

---

### 1.4 ErrorState 重试按钮增加加载态（P0）

**问题**：网络差时点击"重试"，按钮无任何反馈，用户不确定是否触发成功。

**方案**：为 `ErrorState` 增加 `isRetrying` prop，控制按钮 loading 状态。

**涉及文件**：
- `web/src/components/ErrorState.tsx`
- `web/src/pages/EntryListPage.tsx`
- `web/src/pages/EntryDetailPage.tsx`

**关键改动**：
```tsx
// ErrorState.tsx
interface Props {
  message?: string;
  onRetry?: () => void;
  isRetrying?: boolean;
}

export default function ErrorState({ message = '加载失败', onRetry, isRetrying }: Props) {
  // ...
  {onRetry && (
    <Button variant="outline" size="sm" onClick={onRetry} disabled={isRetrying} className="rounded-lg">
      {isRetrying ? <Loader2 size={14} className="mr-2 animate-spin" /> : null}
      重试
    </Button>
  )}
}
```

---

### 1.5 批量操作栏视觉层级强化（P0）

**问题**：批量操作栏虽然已有阴影，但缺少与内容的明确分隔；且选中计数已经存在，只是字号偏小。

**方案**：
1. 阴影增强（已有，可略调）
2. 选中计数增加"已选择"前缀，提升可读性

**涉及文件**：
- `web/src/pages/EntryListPage.tsx:428-432`

**关键改动**：
```tsx
<span className="text-sm font-semibold shrink-0 tabular-nums text-foreground">
  已选择 {selectedIds.size} 项
</span>
```

---

### 1.6 扩展 Popup 最低成本暗色模式支持（P0）

**问题**：扩展完全无暗色模式，在深色系统主题下极其突兀。

**方案**：在 `styles.css` 中增加 `prefers-color-scheme: dark` 媒体查询，将 HEX 色值映射为暗色对应值。不改动组件代码，纯 CSS 方案。

**涉及文件**：
- `extension/src/popup/styles.css`

**关键改动**：
```css
@media (prefers-color-scheme: dark) {
  :root {
    --background: #020617;
    --foreground: #f8fafc;
    --card: #0f172a;
    --card-foreground: #f8fafc;
    --primary: #6366f1;
    --primary-foreground: #ffffff;
    --secondary: #1e293b;
    --secondary-foreground: #f8fafc;
    --muted: #1e293b;
    --muted-foreground: #94a3b8;
    --accent: #1e293b;
    --accent-foreground: #f8fafc;
    --border: #1e293b;
    --input: #1e293b;
    --ring: #6366f1;
  }

  .form-group input::placeholder {
    color: #64748b;
  }

  .message.error {
    background: #450a0a;
    color: #fda4af;
    border-color: #7f1d1d;
  }
  .message.success {
    background: #022c22;
    color: #6ee7b7;
    border-color: #065f46;
  }
  .message.info {
    background: #1e1b4b;
    color: #a5b4fc;
    border-color: #312e81;
  }
}
```

---

## 阶段二：移动端体验专项

**目标**：解决触摸目标尺寸、手势冲突、 affordance 不足等移动端核心问题。预期 3–5 天。

### 2.1 列表卡片操作按钮触控区域扩大（P1）

**问题**：`EntryCard` 中 `h-7 w-7`（28px）的 icon button 远低于 Apple HIG 44pt / Material 48dp 标准。

**方案**：不破坏视觉大小，通过增加不可见 hit-area padding，或直接将移动端按钮提升到 `h-9 w-9`。

**涉及文件**：
- `web/src/components/EntryCard.tsx`

**关键改动**：
```tsx
// 方案 A：视觉保持 28px，触控区域扩大到 36px（推荐）
<Button
  variant="ghost"
  size="icon"
  className={cn(
    'h-9 w-9 rounded-md transition-colors -m-1 p-1',
    // ... 条件样式
  )}
>
  <Star size={15} className={cn(entry.is_starred && 'fill-current')} />
</Button>

// 或方案 B：移动端使用 h-9 w-9
className={cn(
  'h-7 w-7 sm:h-9 sm:w-9 rounded-md transition-colors',
  // ...
)}
```

> 推荐方案 A，因为卡片右侧操作区空间充裕，直接扩大视觉尺寸更清晰。

---

### 2.2 文章详情页手势冲突解决（P1）

**问题**：左右滑动切文章与内容区横向滚动（表格、代码块）冲突。

**方案**：在 `useSwipe` 触发前检测事件目标是否在 `<pre>`、`<table>`、`<code>` 内，如果是则放弃手势接管。

**涉及文件**：
- `web/src/hooks/useSwipe.ts`

**关键改动**：
```ts
// useSwipe.ts 中在 touchstart 处理时增加
const isScrollableContent = (target: HTMLElement) => {
  const scrollable = target.closest('pre, table, code, .no-swipe');
  if (scrollable) return true;
  // 同时检测元素自身是否有横向溢出
  if (target.scrollWidth > target.clientWidth) return true;
  return false;
};
```

---

### 2.3 下拉刷新增加物理反馈动画（P1）

**问题**：当前下拉刷新仅显示文字"下拉刷新"，没有弹性位移和旋转指示器的物理感。

**方案**：让刷新指示器跟随手指下拉距离产生位移和旋转，类似原生 iOS 效果。

**涉及文件**：
- `web/src/pages/EntryListPage.tsx`
- `web/src/hooks/useSwipe.ts`（需返回 `swipeOffset.y`）

**关键改动**：
```tsx
// EntryListPage.tsx
const { swipeOffset, isSwiping: isPulling, ref: refreshRef } = useSwipe(
  { onSwipeDown: handleRefresh },
  { threshold: 60, direction: 'vertical' },
);

// 刷新指示器改为固定顶部，跟随位移
{(isPulling || isRefreshing) && (
  <div
    className="flex items-center justify-center text-sm text-muted-foreground transition-transform"
    style={{
      height: Math.min(swipeOffset.y, 80),
      opacity: Math.min(swipeOffset.y / 60, 1),
    }}
  >
    <Loader2
      size={16}
      className={cn('mr-2', isRefreshing && 'animate-spin')}
      style={{ transform: `rotate(${(swipeOffset.y / 60) * 360}deg)` }}
    />
    {isRefreshing ? '刷新中...' : '下拉刷新'}
  </div>
)}
```

---

### 2.4 详情页标题编辑 affordance（P1）

**问题**：移动端无法 hover，用户不知道标题可点击编辑。

**方案**：在移动端（`lg:hidden`）标题右侧常驻一个 subtle 的铅笔图标。

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`

**关键改动**：
```tsx
<h1
  className="text-xl sm:text-[1.75rem] font-bold mb-3 cursor-pointer hover:text-primary group leading-tight tracking-tight flex items-start gap-2"
  onClick={() => { setTitleDraft(entry.title || ''); setEditingTitle(true); }}
  title="点击编辑标题"
>
  {entry.title || '无标题'}
  <span className="text-sm font-normal text-muted-foreground ml-2 opacity-0 group-hover:opacity-100 transition-opacity lg:opacity-0">
    编辑
  </span>
  {/* 移动端常驻图标 */}
  <Edit3 size={14} className="text-muted-foreground/40 mt-1.5 shrink-0 lg:hidden" />
</h1>
```

---

### 2.5 底部导航 Active 态强化（P1）

**问题**：虽然 `MobileBottomNav` 已有 `bg-primary/10` 背景，但在亮色模式下对比度仍然偏弱。

**方案**：增加 `strokeWidth` 变化（已有）+ 图标容器背景色饱和度微调。同时增加顶部上滑指示器的手势提示（可选）。

**涉及文件**：
- `web/src/components/layout/MobileBottomNav.tsx`

**当前状态已较好**，建议仅微调：
```tsx
// active 态背景从 primary/10 提升到 primary/[0.12]，并增加 subtle 内阴影
active && 'bg-primary/[0.12] shadow-[inset_0_0_0_1px_rgba(79,70,229,0.08)]'
```

---

## 阶段三：交互深度打磨

**目标**：提升专业效率工具的交互质感。预期 5–7 天。

### 3.1 批注面板展开/收起过渡动画（P1）

**问题**：桌面端批注面板是直接条件渲染（`showAnnotations && <AnnotationsSidebar />`），无宽度过渡，正文瞬间重排。

**方案**：用 `AnimatePresence` + CSS Grid 或固定宽度容器实现平滑展开。

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`

**关键改动**：
```tsx
// 桌面端包裹在动画容器中
<div className={cn(
  "hidden lg:block overflow-hidden transition-all duration-300 ease-out",
  showAnnotations ? "w-80 opacity-100 ml-6" : "w-0 opacity-0 ml-0"
)}>
  {showAnnotations && id && <AnnotationsSidebar ... />}
</div>
```

> 注意：`overflow-hidden` + 宽度过渡可以避免布局抖动。`AnnotationsSidebar` 内部需要 `min-w-[20rem]` 防止内容被压缩。

---

### 3.2 键盘快捷键帮助面板（P1）

**问题**：`j`/`k`、`Enter`/`o` 等快捷键对用户完全不可见，新用户无法发现。

**方案**：增加 `Cmd/Ctrl + /` 或 `?` 呼出的快捷键速查浮层（Command Palette 风格）。

**涉及文件**：
- 新增 `web/src/components/KeyboardHelp.tsx`
- `web/src/hooks/useKeyboardShortcuts.ts`
- `web/src/App.tsx` 或 `Layout.tsx`

**设计草案**：
- 触发：`?` 键（不在输入框内时）
- 样式：居中 Dialog，快捷键用 `<kbd>` 标签展示，背景 `bg-muted`、圆角、monospace 字体
- 内容分区：列表导航 / 文章阅读 / 全局操作

---

### 3.3 路由切换焦点管理（P2）

**问题**：React Router 切换页面后，焦点停留在旧页面的链接上，屏幕阅读器用户会困惑。

**方案**：每次路由切换后，将焦点移至页面主标题 `<h1>` 或 `<main>` 区域。

**涉及文件**：
- 新增 `web/src/components/FocusManager.tsx`
- `web/src/App.tsx`

**关键改动**：
```tsx
// FocusManager.tsx
import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

export function FocusManager() {
  const { pathname } = useLocation();
  useEffect(() => {
    const main = document.querySelector('main');
    if (main) {
      main.setAttribute('tabIndex', '-1');
      main.focus({ preventScroll: true });
    }
  }, [pathname]);
  return null;
}
```

---

### 3.4 Skeleton 语义化增强（P2）

**问题**：加载中的 Skeleton 对屏幕阅读器是"静默空白"。

**方案**：给主要加载区域增加 `aria-busy` + `aria-label`，并为列表加载增加情感化微文案。

**涉及文件**：
- `web/src/pages/EntryListPage.tsx`
- `web/src/pages/EntryDetailPage.tsx`
- `web/src/components/ui/skeleton.tsx`

**关键改动**：
```tsx
// EntryListPage.tsx loading 区域
<div className="space-y-3" role="status" aria-live="polite" aria-label="正在加载文章列表">
  {[1, 2, 3].map((i) => (
    <div key={i} className="bg-card border border-border/50 rounded-xl p-5 animate-pulse">
      {/* ... */}
    </div>
  ))}
</div>
```

---

### 3.5 列表多选模式选中态强化（P2）

**问题**：多选模式下卡片选中态仅有 `ring-2 ring-primary/30`，不够显著。

**方案**：增加背景色变化和更粗的 ring。

**涉及文件**：
- `web/src/components/EntryCard.tsx`

**关键改动**：
```tsx
className={cn(
  'group bg-card border rounded-xl overflow-hidden card-hover transition-all',
  selected ? 'ring-2 ring-primary/50 shadow-md shadow-primary/5 bg-primary/[0.03]' : 'border-border/60',
  // ...
)}
```

---

## 阶段四：设计系统统一（扩展端重构）

**目标**：弥合 Extension 与 Web 应用在视觉和技术栈上的断裂。预期 7–10 天。

### 4.1 扩展接入共享 Tailwind 配置（P2）

**问题**：扩展完全手写 CSS，无法复用 Web 的 design token，维护成本高。

**方案**：让扩展构建流程引入 Tailwind CSS，共享 `web/src/index.css` 中的 `@theme inline` 变量。由于扩展 UI 非常简单，可以只编译用到的类，产物体积增加极小。

**涉及文件**：
- `extension/vite.config.ts`
- 新增 `extension/tailwind.config.js`（或 v4 的 CSS import）
- `extension/src/popup/styles.css`（重构）
- `extension/src/popup/main.tsx`

**实施步骤**：
1. 在 extension 目录安装 `tailwindcss`（与 web 同版本）
2. 创建 `extension/src/index.css`，`@import "tailwindcss"` 并共享 Web 的 token
3. `popup/main.tsx` 引入新 CSS
4. 将 `App.tsx` 中的手写 className 逐步替换为 Tailwind 工具类

---

### 4.2 扩展组件库对齐（P2）

**问题**：扩展的 Button、Input、Tabs 均为手写 div，无无障碍支持。

**方案**：将 Web 端的基础组件抽离为"纯样式组件"（headless 或仅依赖 Radix），供扩展复用。考虑到扩展体积，可以只引入最简封装。

**涉及文件**：
- `web/src/components/ui/button.tsx`（确认无额外依赖后可复用）
- `web/src/components/ui/input.tsx`
- `extension/src/popup/App.tsx`

**替代方案**（更低成本）：手写组件增加 ARIA 属性：
```tsx
// 扩展中的 Tabs
<div role="tablist" className="tabs">
  <button role="tab" aria-selected={loginTab === 'password'} ... />
</div>
```

---

### 4.3 扩展增加中文字体回退栈（P3）

**问题**：扩展字体栈仅 `"Inter", -apple-system...`，中文环境可能 fallback 到不理想的系统字体。

**方案**：与 Web 端保持一致：
```css
font-family: "Inter", "Noto Sans SC", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", ui-sans-serif, system-ui, sans-serif;
```

**涉及文件**：
- `extension/src/popup/styles.css`

---

### 4.4 扩展动画系统补全（P3）

**问题**：扩展仅有 spinner 旋转，视图切换生硬。

**方案**：增加视图切换的 fade-in / slide-in 动画。

**涉及文件**：
- `extension/src/popup/styles.css`

**关键改动**：
```css
@keyframes fade-in-up {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
.view-enter {
  animation: fade-in-up 0.25s ease-out;
}
```

---

## 阶段五：架构级体验重构

**目标**：对核心阅读流程进行结构性优化。视需求排期。

### 5.1 文章详情页操作栏重构（P3）

**问题**：当前 `[标题] → [元信息] → [操作栏] → [正文]` 结构中，操作栏占据了阅读黄金位置，且低频操作（重抓、编辑）与高频操作（收藏、批注）混在一起。

**方案 A：右侧 Sticky 工具栏（Desktop）**
- Desktop：标题右侧放置垂直 sticky 工具栏（Star / Archive / Annotate / More）
- Mobile：保持底部 toolbar 或改为 FAB

**方案 B：操作栏精简**
- 仅保留 Star / Archive / Annotate 外露
- 重抓、编辑、删除收拢到 `MoreHorizontal` DropdownMenu

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`
- `web/src/index.css`

---

### 5.2 标签位置上移（P3）

**问题**：`EntryTags` 位于文章最底部，用户需要滚动到文末才能查看或编辑。

**方案**：将标签区域上移至元信息下方、操作栏上方，或与操作栏同行（Desktop 空间允许时）。

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`

---

### 5.3 内容提取失败状态的体验优化（P2）

**问题**：提取失败时仅显示一行红色文字+"查看原文"链接，用户不知道接下来该做什么。

**方案**：失败状态升级为小型卡片，提供明确的后续操作：
- "查看原文"（主按钮）
- "重新抓取"（次要按钮）
- 失败原因提示（如"网站禁止爬虫"）

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx:400-404`

---

## 附录：优先级速查表

| 任务 | 阶段 | 优先级 | 工时 | 影响面 |
|------|------|--------|------|--------|
| 删除确认 Dialog 统一 | 一 | P0 | 30min | 全平台一致性 |
| 操作成功 Toast 补全 | 一 | P0 | 20min | 操作反馈感知 |
| 划词工具栏箭头 | 一 | P0 | 20min | 批注体验 |
| ErrorState 加载态 | 一 | P0 | 20min | 弱网体验 |
| 批量操作栏文案 | 一 | P0 | 5min | 多选清晰度 |
| 扩展暗色模式 | 一 | P0 | 40min | 扩展一致性 |
| 列表操作按钮触控扩大 | 二 | P1 | 30min | 移动端可用性 |
| 手势冲突解决 | 二 | P1 | 1h | 移动端阅读流畅度 |
| 下拉刷新物理动画 | 二 | P1 | 1.5h | 移动端原生感 |
| 标题编辑 affordance | 二 | P1 | 15min | 移动端可发现性 |
| 批注面板过渡动画 | 三 | P1 | 1.5h | 桌面端质感 |
| 快捷键帮助面板 | 三 | P1 | 3h | 效率工具专业感 |
| 路由焦点管理 | 三 | P2 | 1h | 可访问性 |
| Skeleton 语义化 | 三 | P2 | 1h | 可访问性 |
| 多选选中态强化 | 三 | P2 | 15min | 视觉反馈 |
| 扩展接入 Tailwind | 四 | P2 | 4h | 维护成本 |
| 扩展组件 ARIA | 四 | P2 | 2h | 可访问性 |
| 操作栏重构 | 五 | P3 | 4h | 阅读沉浸感 |
| 标签位置上移 | 五 | P3 | 1h | 信息架构 |
| 提取失败状态升级 | 五 | P2 | 1.5h | 容错体验 |

---

## 实施建议

1. **阶段一可在一个 PR 内完成**，改动分散但均为局部修改，风险极低。
2. **阶段二建议单独一个 PR**，涉及 `useSwipe` 等核心 hook 的改动，需要充分测试移动端真机。
3. **阶段三可拆分为 2–3 个 PR**， KeyboardHelp 和 FocusManager 是新增功能，可独立交付。
4. **阶段四建议作为一个独立的技术债务 PR**，避免与功能改动混在一起。
5. **阶段五作为体验提升 backlog**，可在后续版本中按需取用。
