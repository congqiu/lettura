# 移动端体验优化设计规格

## 背景

Lettura 已有基础的移动端适配（viewport meta、PWA、底部导航、sidebar-as-sheet），但存在多处功能缺陷和体验短板。本规格覆盖从 bug 修复到触摸手势增强的完整优化。

## 修复项

### M1. AnnotationsSidebar 移动端适配

**问题**：`AnnotationsSidebar.tsx` 使用固定 `w-80 border-l` 布局，在移动端会挤压文章内容或导致水平溢出，完全不可用。

**方案**：
- 在 `EntryDetailPage` 层面做移动/桌面分支渲染，而非将 Sheet 逻辑塞进 AnnotationsSidebar：
  - 移动端：`<Sheet open={showAnnotations} onOpenChange={setShowAnnotations}><SheetContent side="bottom" className="h-[60dvh]"><AnnotationsSidebar compact /></SheetContent></Sheet>`
  - 桌面端：`{showAnnotations && <AnnotationsSidebar />}`
- `AnnotationsSidebar` 接收 `compact?: boolean` prop：移动端去掉 `w-80 border-l`，改为全宽布局
- 现有批注按钮改为 `hidden lg:inline-flex`（桌面端可见）
- 新增批注按钮 `lg:hidden`（移动端可见），点击 `setShowAnnotations(true)` 打开底部 Sheet
- `showAnnotations` 状态同时控制两种模式：移动端控制 Sheet 的 `open`，桌面端控制右侧面板的渲染
- Sheet 高度使用 `60dvh`（动态视口高度），适配移动端地址栏收缩

**涉及文件**：
- `web/src/components/AnnotationsSidebar.tsx`
- `web/src/pages/EntryDetailPage.tsx`

### M2. EntryDetailPage 动态 Tailwind 类名修复

**问题**：`EntryDetailPage.tsx` 第 75 行使用 `lg:${showAnnotations ? 'max-w-3xl' : 'max-w-3xl'}` 运行时拼接类名。Tailwind 无法静态检测动态类名，`lg:max-w-3xl` 不会出现在编译后的 CSS 中。当前没有可见症状是因为 `Layout.tsx` 第 24 行已使用静态 `lg:max-w-3xl`，Tailwind 已生成该 utility class。若 Layout 的该类被删除，此处样式会静默失效。

**方案**：
- 替换为静态类名：直接写 `lg:max-w-3xl`（两个分支结果相同，无需条件）
- 全局搜索其他类似的动态拼接模式（正则：`(?:lg|md|sm|xl|2xl):\$\{`）并一并修复

**涉及文件**：
- `web/src/pages/EntryDetailPage.tsx`
- 全局检查其他文件

### M3. 批量操作栏与底部导航重叠

**问题**：`EntryListPage.tsx` 的批量操作栏（fixed bottom）与 `MobileBottomNav`（fixed bottom）在移动端互相遮挡。

**方案**：
- MobileBottomNav 渲染时通过 `useLayoutEffect` 测量自身高度，设置 CSS 变量 `--bottom-nav-height` 到 `document.documentElement`
- 批量操作栏在移动端使用 `bottom-[var(--bottom-nav-height)]` 动态定位，而非硬编码 `bottom-16`
- 批量操作栏加 `pb-[env(safe-area-inset-bottom)]` 处理安全区域
- 桌面端不受影响（底部导航已 `lg:hidden`，CSS 变量不生效时 fallback 到 `bottom-0`）

**涉及文件**：
- `web/src/pages/EntryListPage.tsx`
- `web/src/components/layout/MobileBottomNav.tsx`

### M4. Settings 页表格移动端适配

**问题**：`SettingsPage.tsx` 的标签管理表格使用标准 `<table>`，在小屏幕上水平溢出。

**方案**：
- 移动端（<640px）：表格行转为卡片布局，每行一个卡片
- 卡片字段排列：标签名（大字）→ 文章数（小字灰色）→ 操作按钮（卡片底部，水平排列：编辑/删除）
- 编辑状态下：Input 替换标签名位置，保存/取消按钮替换操作按钮位置
- 桌面端：保持现有表格布局
- 用 `hidden sm:block` / `sm:hidden` 切换两套渲染
- 标签规则列表当前已是卡片式 `flex items-center` 布局，无需改动

**涉及文件**：
- `web/src/pages/SettingsPage.tsx`

### M5. Web Share Target API

**问题**：设计规格要求支持 Web Share Target API，但 `manifest.webmanifest` 中缺少 `share_target` 字段，用户无法从手机浏览器分享菜单直接保存链接。

**方案**：
- `vite.config.ts` 的 VitePWA 配置中添加 `share_target`：
  ```json
  {
    "action": "/share-target",
    "method": "GET",
    "enctype": "application/x-www-form-urlencoded",
    "params": {
      "url": "url",
      "text": "text"
    }
  }
  ```
- 新增 `web/src/pages/ShareTargetPage.tsx`：
  - 解析 URL 参数中的 `url` / `text` 字段
  - URL 提取策略：优先使用 `url` 参数；若为空则从 `text` 中用正则提取第一个 `https?://` URL；若均无有效 URL 则显示提示页面"未检测到链接"
  - URL 验证：必须是 `http://` 或 `https://` 协议
  - 调用 `createEntry` API 保存
  - 保存成功：显示"已保存"反馈，2 秒后跳转到文章详情页
  - URL 已存在（409）：显示"该链接已保存"，提供跳转到已有文章的链接
  - 保存失败：显示错误信息，提供重试按钮
- **路由与认证处理**：
  - `/share-target` 路由放在 `<ProtectedRoute>` 之外，作为独立路由
  - ShareTargetPage 自行检测认证状态：未登录时将 URL 参数存入 `sessionStorage`，重定向到 `/login?redirect=/share-target`
  - 登录页面读取 `redirect` 参数，登录成功后跳转回原始 URL
  - ShareTargetPage 在 mount 时检查 `sessionStorage` 中是否有暂存的分享数据（登录后恢复场景）
- 路由添加 `/share-target` 路径

**涉及文件**：
- `web/vite.config.ts`
- `web/src/pages/ShareTargetPage.tsx`（新增）
- `web/src/App.tsx`（路由）
- `web/src/pages/LoginPage.tsx`（支持 redirect 参数）

### M6. safe-area-inset-top 缺失

**问题**：Layout.tsx 的移动端 header 没有处理 iPhone 刘海/状态栏区域，内容可能被遮挡。

**前置条件**：`env(safe-area-inset-top)` 仅在 viewport meta 包含 `viewport-fit=cover` 时生效。需先确认 `web/index.html` 的 viewport meta 是否已包含该值，若未包含则添加。

**方案**：
- viewport meta 修改为：`<meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />`
- 移动端 header 加 `pt-[env(safe-area-inset-top)]`
- 检查其他 fixed 顶部元素，统一处理

**涉及文件**：
- `web/index.html`
- `web/src/components/Layout.tsx`

### M7. Pages 页 tab 栏滚动

**问题**：`PagesPage.tsx` 的 5 个 tab 在窄屏可能溢出。

**方案**：
- tab 容器加 `overflow-x-auto flex-nowrap`
- 在 `web/src/index.css` 中用 Tailwind v4 的 `@utility` 指令添加 `scrollbar-hide` 工具类：
  ```css
  @utility scrollbar-hide {
    scrollbar-width: none;
    &::-webkit-scrollbar {
      display: none;
    }
  }
  ```

**涉及文件**：
- `web/src/pages/PagesPage.tsx`
- `web/src/index.css`

### M8. 选择框裁切修复

**问题**：`EntryListPage.tsx` 的选择框使用 `absolute left-0 top-5 -ml-8`，在移动端可能超出屏幕左侧被裁切。选择框当前在 EntryListPage 中渲染，位于 EntryCard 外层 DOM。

**方案**：
- 将选择框移入 EntryCard 组件内部，EntryCard 接收 `selectionMode: boolean` / `selected: boolean` / `onToggleSelect: () => void` props
- 移动端：EntryCard 内部用 flex 布局，选择框在左侧，内容在右侧，无负 margin
- 桌面端：保持现有 absolute 定位样式（`absolute left-0 top-5 -ml-8`），通过 `hidden lg:block` / `lg:hidden` 切换两套选择框渲染
- 或者统一使用 flex 布局，桌面端也改为正常流，去掉 absolute 定位（更简洁）

**涉及文件**：
- `web/src/pages/EntryListPage.tsx`
- `web/src/components/EntryCard.tsx`

## 触摸手势

### M9. 通用 useSwipe hook

**方案**：
- 新增 `web/src/hooks/useSwipe.ts`
- 封装 `touchstart` / `touchmove` / `touchend` 事件处理
- 配置项：`threshold`（触发阈值，px，默认 80）、`direction`（'horizontal' | 'vertical' | 'all'，默认 'horizontal'）
- 返回：
  - `onSwipeLeft` / `onSwipeRight` / `onSwipeUp` / `onSwipeDown` 回调
  - `swipeOffset: { x: number, y: number }` — 实时偏移量，用于视觉反馈
  - `swipingDirection: 'left' | 'right' | 'up' | 'down' | null` — 当前滑动方向状态
  - `isSwiping: boolean` — 是否正在滑动
- 事件处理策略：
  - `addEventListener` 使用 `{ passive: false }` 以支持 `preventDefault()`
  - 方向锁定：移动 10px 后锁定方向，后续 touchmove 只处理锁定方向的偏移
  - 锁定为水平方向时 `preventDefault()` 阻止浏览器水平手势（如前进/后退），允许垂直滚动
  - 锁定为垂直方向时不 `preventDefault()`，允许正常滚动
  - 忽略短距离误触（<10px 不触发方向锁定）
- 单元测试：使用 jsdom 环境，模拟 TouchEvent（需构造 `Touch` 对象和 `TouchEvent`），测试方向锁定、阈值触发、回调调用

### M10. 左滑返回

**场景**：文章详情页左滑返回列表。

**方案**：
- `EntryDetailPage.tsx` 使用 `useSwipe` hook
- **仅从屏幕左边缘 30px 区域开始**的左滑触发返回（类似 iOS 边缘返回手势），与 M12 的内容区域左右切换不冲突
- 左滑距离 >80px 触发 `navigate(-1)`
- 视觉反馈：页面跟随手指向右偏移（`transform: translateX(${swipeOffset.x}px)`），松手后若未达阈值则弹回（CSS transition）
- 仅在移动端启用（`useIsMobile()` 判断）

### M11. 下拉刷新

**场景**：EntryListPage 下拉刷新列表内容。

**方案**：
- `EntryListPage.tsx` 使用 `useSwipe` hook（direction: 'vertical'）
- 下拉距离 >60px 触发刷新
- 使用 `qc.invalidateQueries({ queryKey: ['entries-infinite'] })` 而非 `refetch()`，确保所有页面数据都被标记为过期并重新加载
- 刷新指示器：顶部显示旋转图标 + "刷新中..."，刷新完成后自动收起
- 仅在页面滚动到顶部时（`scrollTop === 0`）启用下拉刷新，避免与正常滚动冲突
- 仅在移动端启用

### M12. 左右滑动切换文章

**场景**：文章详情页左右滑切换上/下篇文章。

**方案**：
- `EntryDetailPage.tsx` 使用 `useSwipe` hook
- 右滑 → 上一篇，左滑 → 下一篇（从屏幕内容区域开始，与 M10 的边缘返回不冲突）
- 滑动距离 >100px 触发切换
- **列表上下文获取**：通过路由 state 传递 `entryIds: string[]` 和 `currentIndex: number`
  - 修改所有导航到详情页的入口，在 `navigate()` 时携带 `{ state: { entryIds, currentIndex } }`
  - 需修改的入口：EntryCard 的 `<Link>`、useKeyboardShortcuts 的 `navigate()`、搜索结果的点击
  - 无限滚动场景：entryIds 只包含已加载的 entries，到达边界时（第一篇/最后一篇）滑动无效果
  - 无列表上下文时（如从通知/分享直接进入）：滑动切换不可用，不显示提示
- 切换时带页面滑动动画（CSS transition），新文章从对应方向滑入
- 仅在移动端启用

**涉及文件**：
- `web/src/hooks/useSwipe.ts`（新增）
- `web/src/pages/EntryDetailPage.tsx`
- `web/src/pages/EntryListPage.tsx`
- `web/src/components/EntryCard.tsx`（导航时传递列表上下文）
- `web/src/hooks/useKeyboardShortcuts.ts`（导航时传递列表上下文）

## 测试策略

- **M5 ShareTargetPage**：单元测试 URL 提取逻辑（从 text 中提取 URL、验证协议、处理空参数）
- **M9 useSwipe hook**：单元测试方向锁定、阈值触发、回调调用（jsdom 环境模拟 TouchEvent）
- 其他项以手动测试为主，在移动端浏览器和 DevTools 设备模拟器中验证

## 实施顺序

1. M2（Tailwind bug 修复）— 最简单，立即见效
2. M6（safe-area-inset-top）— 简单修复
3. M8（选择框裁切）— 简单修复
4. M7（tab 栏滚动）— 简单修复
5. M3（批量操作栏重叠）— 中等修复
6. M4（Settings 卡片布局）— 中等改动
7. M1（AnnotationsSidebar 适配）— 较大改动
8. M5（Web Share Target）— 新增页面
9. M9（useSwipe hook）— 基础设施
10. M10（左滑返回）— 依赖 M9
11. M11（下拉刷新）— 依赖 M9
12. M12（滑动切换文章）— 依赖 M9，最复杂

## 不做的事

- 不做离线 API 缓存（PWA spec 已明确排除）
- 不做原生 App
- 不做 i18n
- 不做复杂的滑动手势自定义设置界面
