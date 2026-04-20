# shadcn/ui 暖色主题前端重设计

**日期**: 2026-04-20
**状态**: 待实施

## 概述

引入 shadcn/ui (New York 风格变体) 替代当前纯 Tailwind 手写组件，全面重构前端样式和布局。采用温暖柔和的视觉风格，响应式侧边栏导航，统一设计系统。

## 设计决策

### 视觉风格: 温暖柔和

- 暖色调 (amber/stone 色板)，大圆角，低对比度
- 类似 Bear、Calm 风格，适合阅读类应用
- 浅色模式: 米黄底 (#fefcf3) + 白色卡片 + 深棕文字 (#451a03)
- 暗色模式: 深棕暖灰底 (#1c1917) + 深棕卡片 (#292524) + 米黄文字 (#fefcf3)

### 布局架构: 响应式侧边栏

**桌面端 (≥1024px):**
- 左侧固定 220px 侧边栏: Logo、导航项、未读计数 Badge、设置、主题切换
- 右侧内容区: 可滚动，最大宽度约 800px 居中

**移动端 (<1024px):**
- 顶部导航栏: Logo + 搜索 + 设置图标
- 底部 Tab 导航: 未读、归档、收藏、便签、更多

**文章详情页:**
- 保持侧边栏不变
- 内容区展示文章，操作按钮改为 Pill/Badge 风格

### 列表布局: 卡片列表

保持当前卡片式列表，但样式暖色化:
- 圆角 12px 卡片，暖色边框
- 缩略图保持右侧展示
- 操作按钮用图标 + shadcn Button ghost variant

## 配色系统

### CSS 变量 (shadcn/ui 格式)

**浅色模式:**
```css
:root {
  --background: 30 50% 97%;      /* #fefcf3 */
  --foreground: 20 60% 10%;      /* #451a03 */
  --card: 0 0% 100%;             /* #ffffff */
  --card-foreground: 20 60% 10%; /* #451a03 */
  --popover: 0 0% 100%;
  --popover-foreground: 20 60% 10%;
  --primary: 30 90% 45%;         /* #d97706 */
  --primary-foreground: 0 0% 100%;
  --secondary: 40 40% 90%;       /* #f5f0e1 */
  --secondary-foreground: 20 60% 10%;
  --muted: 40 30% 85%;
  --muted-foreground: 30 30% 40%; /* #a16207 */
  --accent: 40 40% 90%;
  --accent-foreground: 20 60% 10%;
  --destructive: 20 70% 40%;     /* #b45309 */
  --destructive-foreground: 0 0% 100%;
  --border: 30 20% 80%;          /* #e8e0c8 */
  --input: 30 20% 80%;
  --ring: 30 90% 45%;
  --radius: 0.625rem;            /* 10px */
}
```

**暗色模式:**
```css
.dark {
  --background: 20 10% 10%;       /* #1c1917 */
  --foreground: 30 50% 97%;       /* #fefcf3 */
  --card: 10 5% 15%;              /* #292524 */
  --card-foreground: 30 50% 97%;
  --popover: 10 5% 15%;
  --popover-foreground: 30 50% 97%;
  --primary: 40 90% 55%;          /* #fbbf24 */
  --primary-foreground: 20 10% 10%;
  --secondary: 10 5% 18%;
  --secondary-foreground: 30 50% 97%;
  --muted: 20 5% 25%;
  --muted-foreground: 30 10% 65%; /* #a8a29e */
  --accent: 10 5% 18%;
  --accent-foreground: 30 50% 97%;
  --destructive: 0 60% 50%;       /* #dc2626 */
  --destructive-foreground: 0 0% 100%;
  --border: 20 5% 25%;            /* #44403c */
  --input: 20 5% 25%;
  --ring: 40 90% 55%;
}
```

## 组件映射

### 直接替换 (12 个)

| 当前 | shadcn 组件 | 说明 |
|------|------------|------|
| 所有 `<button>` | Button | variant: default/outline/ghost/destructive |
| 搜索框、登录输入框 | Input | 统一样式 |
| ConfirmDialog | AlertDialog | 基于 Radix AlertDialog |
| Toast 通知 | Sonner | 替代手写 Toast 实现 |
| MobileDrawer | Sheet | 侧滑面板 |
| 标签筛选 | Tabs | 基于 Radix Tabs |
| EntryTags 标签 | Badge | 标签交互状态 |
| 收藏/归档/更多操作 | DropdownMenu | 操作菜单 |
| PageUploadModal, PageEditModal | Dialog | 基于 Radix Dialog |
| 键盘快捷键帮助 | Command | cmd+k 命令面板 |
| Loading spinner | Skeleton | 内容骨架屏 |
| AddEntryForm URL 输入 | Input + Button | 组合输入框 |

### 保留并优化 (5 个)

| 组件 | 处理方式 |
|------|---------|
| EntryCard | 暖色 Card 样式 + shadcn Button/Badge |
| ContentEditor | Tiptap 保持，工具栏用 shadcn Toggle |
| AnnotationsSidebar | 保持自定义，暖色 Card 包裹 |
| ThemeToggle | shadcn DropdownMenu 实现 |
| EmptyState/ErrorState | 暖色重新样式化 |

### 新增 (2 个)

| 组件 | 用途 |
|------|------|
| Sidebar | 桌面端侧边栏导航 |
| Separator | 替代手写 border-b 分隔线 |

## 技术方案

### shadcn/ui 安装

使用 shadcn/ui CLI 初始化 (New York 风格)，然后逐个添加组件:

```bash
npx shadcn@latest init
# 选择: New York, Tailwind v4, CSS variables

npx shadcn@latest add button input badge dialog alert-dialog
npx shadcn@latest add dropdown-menu tabs sheet separator sonner
npx shadcn@latest add skeleton sidebar command
```

### Tailwind v4 兼容性

shadcn/ui 已支持 Tailwind v4。组件文件使用 CSS 变量方式引用颜色 (如 `bg-background`, `text-foreground`)，主题通过 `index.css` 中的 CSS 变量控制。

### 文件结构变化

```
web/src/
├── components/
│   ├── ui/                    # shadcn 组件 (自动生成)
│   │   ├── button.tsx
│   │   ├── input.tsx
│   │   ├── dialog.tsx
│   │   ├── alert-dialog.tsx
│   │   ├── badge.tsx
│   │   ├── tabs.tsx
│   │   ├── sheet.tsx
│   │   ├── dropdown-menu.tsx
│   │   ├── separator.tsx
│   │   ├── skeleton.tsx
│   │   ├── sidebar.tsx
│   │   ├── command.tsx
│   │   └── sonner.tsx
│   ├── layout/                # 新增: 布局组件
│   │   ├── AppSidebar.tsx     # 侧边栏
│   │   ├── MobileNav.tsx      # 移动端底栏 (重构)
│   │   └── Header.tsx         # 移动端顶栏
│   ├── AddEntryForm.tsx       # 重构
│   ├── EntryCard.tsx          # 重构样式
│   ├── ContentEditor.tsx      # 重构工具栏
│   ├── ... (其他组件保持，样式更新)
│   └── Toast.tsx              # 删除，由 Sonner 替代
├── lib/
│   └── utils.ts               # shadcn 需要: cn() 函数
└── index.css                  # 重写: 暖色 CSS 变量主题
```

### 关键依赖变更

```json
{
  "新增": {
    "@radix-ui/react-alert-dialog": "latest",
    "@radix-ui/react-dialog": "latest",
    "@radix-ui/react-dropdown-menu": "latest",
    "@radix-ui/react-tabs": "latest",
    "@radix-ui/react-separator": "latest",
    "cmdk": "latest",
    "sonner": "latest",
    "class-variance-authority": "latest",
    "clsx": "latest",
    "tailwind-merge": "latest",
    "lucide-react": "保持"
  }
}
```

注意: shadcn CLI 会自动安装所有需要的 Radix UI 包。

### 不做的事

- 不引入 shadcn 的表单组件 (项目表单简单，不需要 react-hook-form)
- 不引入 shadcn 的 Table 组件 (没有表格场景)
- 不引入 shadcn 的 Date Picker (不需要)
- 不改变 API 层和状态管理逻辑
- 不改变路由结构

## 影响范围

- 所有 17 个组件文件需要更新样式或重构
- 6 个页面组件需要更新布局
- index.css 完全重写
- Layout.tsx 重构为响应式侧边栏布局
- MobileNav.tsx 重构为底栏导航
- Toast.tsx 删除，由 Sonner 替代

## 实施顺序建议

1. 安装 shadcn/ui + 配置暖色主题 CSS 变量
2. 添加基础 UI 组件 (Button, Input, Badge, Separator)
3. 重构布局 (Sidebar + 响应式导航)
4. 替换交互组件 (Dialog, Sheet, Tabs, DropdownMenu, Sonner)
5. 更新页面组件样式 (EntryListPage, EntryDetailPage, 等)
6. 更新其余页面 (LoginPage, RegisterPage, SettingsPage, MemosPage, PagesPage)
