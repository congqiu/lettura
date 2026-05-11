# UI/UX 优化计划 — Lettura Web 前端

> 基于 2026-05-10 设计审查报告制定，按优先级分 4 个批次执行。

---

## 一、P0 — 立即修复（Bug 级）

### 1.1 修正 MobileBottomNav 图标错误
- **问题**：`moreItems` 中归档误用 `Home`，标签误用 `Star`，操作日志误用 `Home`
- **文件**：`web/src/components/layout/MobileBottomNav.tsx`
- **改动**：
  - 归档 → `Archive`
  - 标签 → `Tag`
  - 操作日志 → `ShieldCheck` 或 `ClipboardList`
- **验收**：Sheet 中三个菜单项显示正确语义图标

### 1.2 统一删除确认使用 AlertDialog
- **问题**：`EntryDetailPage` 使用原生 `confirm()`，与系统 Dialog 风格割裂
- **文件**：`web/src/pages/EntryDetailPage.tsx`
- **改动**：引入 `AlertDialog` 系列组件，替换 `confirm('确定删除这篇文章？')`
- **验收**：删除文章时弹出与批量删除一致的圆角 Dialog，含"取消"和"删除"按钮

---

## 二、P1 — 近期优化（基础体验）

### 2.1 调整 Primary 色相与饱和度
- **问题**：当前 `hsl(220 70% 50%)` 纯蓝过于刺眼，与阅读场景氛围不符
- **文件**：`web/src/index.css`
- **改动**：
  - 浅色模式：`--primary: 222 47% 31%`（深靛蓝）
  - 深色模式：`--primary: 217 80% 65%`（柔和亮蓝）
  - 同步调整 `--ring`、`--sidebar-primary`、`--sidebar-ring`
- **验收**：整体界面不再刺眼，阅读氛围更沉静

### 2.2 提升边框对比度
- **问题**：`border-border/70`、`border-border/60` 导致卡片边界感弱
- **文件**：`web/src/index.css`、`web/src/components/EntryCard.tsx` 等
- **改动**：
  - 全局边框不透明度统一为 100%
  - 或评估采用"无边框卡片 + 背景色差"方案（`bg-card` vs `bg-background`）
- **验收**：列表页卡片边界清晰可见，信息层级明确

### 2.3 空状态视觉增强
- **问题**：`EmptyState` 图标 `text-muted-foreground/50` 几乎不可见
- **文件**：`web/src/components/EmptyState.tsx`
- **改动**：
  - 图标尺寸增大至 32-40px
  - 图标颜色改为 `text-muted-foreground`（去掉 /50）
  - 背景容器可考虑使用品牌色轻量点缀（如 `bg-primary/5`）
- **验收**：空状态有明确的视觉锚点

### 2.4 骨架屏匹配真实结构
- **问题**：`EntryListPage` 骨架屏只是 3 个矩形条，无法建立内容预期
- **文件**：`web/src/pages/EntryListPage.tsx`
- **改动**：骨架屏应模拟 EntryCard 结构——标题条（75% 宽）、元信息条（50% + 30%）、操作区条（20%），有条件时包含缩略图占位
- **验收**：骨架屏与真实卡片结构一致

### 2.5 stagger 动画断档修复
- **问题**：`.stagger-children > *:nth-child(10)` 后无动画定义，长列表出现断层
- **文件**：`web/src/index.css`
- **改动**：
  - 方案 A：增加 `:nth-child(11~20)` 的定义
  - 方案 B（推荐）：改用基于 index 的 JS 动态延迟，或限制 stagger 仅作用于首屏 8-10 项，后续项使用 `animate-fade-in`
- **验收**：长列表滚动加载后，新项也能有平滑出现动画

---

## 三、P2 — 体验升级（交互深度）

### 3.1 页面切换动画升级
- **问题**：`Layout.tsx` 仅 200ms opacity 淡入淡出，路由切换有闪烁感
- **文件**：`web/src/components/Layout.tsx`
- **改动**：引入更丰富的路由过渡：
  - 列表 → 详情：详情页从右侧滑入（`slide-in-right`）
  - 或采用共享元素过渡（若技术成本可控）
- **验收**：页面切换时有方向感，不再闪烁

### 3.2 批量操作栏重设计
- **问题**：底部固定栏中输入框 `w-28` 过窄，移动端几乎不可用
- **文件**：`web/src/pages/EntryListPage.tsx`
- **改动**：
  - 桌面端：横向面板，输入框 `flex-1`（≥200px），核心操作外露
  - 移动端：简化为"全选 + 归档 + 删除"，标签操作通过底部 Sheet/Command 完成
- **验收**：在 375px 宽屏幕上批量操作可用

### 3.3 已读/未读视觉区分
- **问题**：列表中已读和未读文章看起来完全一样
- **文件**：`web/src/components/EntryCard.tsx`
- **改动**：
  - 未读：标题 `font-semibold`，左侧 2px 蓝色竖线指示器，或右上角小蓝点
  - 已读：标题 `font-normal`，整体色调略向 muted 偏移
- **验收**：用户能快速扫描出新内容

### 3.4 阅读进度条
- **问题**：长文章缺乏进度反馈
- **文件**：`web/src/pages/EntryDetailPage.tsx`
- **改动**：在文章顶部（header 下方或页面顶部）增加固定细进度条：`h-0.5 bg-primary`，宽度根据滚动进度动态计算
- **验收**：滚动文章时，顶部进度条实时反映阅读位置

---

## 四、P3 — 长期打磨（设计系统完善）

### 4.1 大屏内容区宽度优化
- **问题**：`max-w-3xl`（768px）在 1440px+ 屏幕上两侧留白过大
- **文件**：`web/src/components/Layout.tsx`、`web/src/pages/EntryDetailPage.tsx`
- **改动**：
  - 列表页保持 `max-w-3xl`（窄行利于扫描）
  - 详情阅读页放宽至 `max-w-3xl` ~ `max-w-4xl`（约 720-896px）
  - 或采用响应式：`lg:max-w-3xl xl:max-w-4xl`
- **验收**：大屏利用率提升，阅读行宽仍保持舒适

### 4.2 阴影系统完善
- **问题**：`card-hover` 阴影几乎不可感知
- **文件**：`web/src/index.css`
- **改动**：引入分层阴影变量：
  - `--shadow-card`: 静态微弱阴影
  - `--shadow-card-hover`: 更明显的中层阴影
  - `--shadow-dropdown`: 弹出层专用阴影
- **验收**：hover 卡片时有明确的"浮起"感知

### 4.3 圆角规则统一
- **问题**：`rounded-xl`、`rounded-lg`、`rounded-2xl`、`rounded-full` 混用无规则
- **文件**：全局组件
- **改动**：建立 3 级圆角体系：
  - `sm`（4px）：tag、badge
  - `md`（8px）：button、input
  - `lg`（12-16px）：card、modal
- **验收**：同一页面中圆角使用有明确层级感

### 4.4 Focus Ring 增强
- **问题**：`ring-ring/50` 在浅色背景下几乎不可见
- **文件**：`web/src/index.css`
- **改动**：提升至 `ring-ring/80` 或 `ring-primary/30`
- **验收**：键盘 Tab 导航时焦点位置清晰可见

---

## 五、执行建议

| 批次 | 预计工时 | 建议启动条件 |
|------|----------|--------------|
| P0 | 30 分钟 | 立即执行，零风险 |
| P1 | 2-3 小时 | P0 完成后，可分批提测 |
| P2 | 4-6 小时 | P1 完成后，涉及交互逻辑需测试覆盖 |
| P3 | 3-4 小时 | 作为设计系统迭代，可独立排期 |

**风险点**：
- Primary 色调整会影响全站按钮、链接、active 状态，需全局回归
- 页面切换动画升级需确保与 `ErrorBoundary`、`Suspense` 兼容
- 已读/未读视觉区分需要后端配合提供 `is_read` 字段（若当前无此字段）
