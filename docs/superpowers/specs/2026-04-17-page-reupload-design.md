# 页面编辑时支持重新上传文件

## 背景

当前页面的编辑功能（`PageEditModal`）只支持修改标题、密码和有效期，不支持替换已上传的文件。用户如果需要修正文件内容，只能删除后重新创建，导致链接（slug）变更。

## 需求

在编辑弹窗中支持重新上传文件，完全替换原有文件（不合并），保留原 slug 和其他设置不变。

## 设计方案

复用已有的 `/pages/upload` 上传接口 + 扩展 `PATCH /pages/{id}` 更新接口。

### 后端改动

#### 1. UpdatePageRequest 增加字段

```rust
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<Option<String>>,
    pub status: Option<String>,
    pub expires_at: Option<Option<String>>,
    // 新增
    pub upload_id: Option<String>,    // 临时上传会话 ID
    pub entry_file: Option<String>,   // 入口文件名
}
```

#### 2. update_page_handler 增加文件替换逻辑

当 `upload_id` 有值时：
1. 校验临时目录 `pages_storage_path/tmp/{upload_id}` 存在
2. 删除旧文件：local 模式清空 `pages_storage_path/{slug}/` 目录后重建；S3 模式删除 `pages/{slug}/` 前缀下所有对象
3. 从临时目录复制新文件到正式目录（复用 `copy_dir_recursive` / `read_dir_recursive` + `storage.store`）
4. 更新 DB 中的 `file_count` 和 `entry_file`
5. 清理临时目录

#### 3. page::UpdatePageParams 增加 entry_file 和 file_count

```rust
pub struct UpdatePageParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub password: Option<Option<String>>,
    pub status: Option<String>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub entry_file: Option<String>,
    pub file_count: Option<i32>,
}
```

UPDATE SQL 增加对 `entry_file` 和 `file_count` 的更新。

### 前端改动

#### 4. PageEditModal 增加文件上传区域

- 在标题字段上方增加一个可折叠的「替换文件」区域
- 支持拖拽和点击选择文件（与 PageUploadModal 相同的交互）
- 上传后显示文件数量和入口文件选择（多 HTML 文件时显示下拉选择）
- 保存时将 `upload_id` 和 `entry_file` 随请求发送

#### 5. updatePage API 函数增加参数

```typescript
updatePage(id, {
  title, password, status, expires_at,
  upload_id,   // 新增
  entry_file,  // 新增
})
```

## 数据流

```
用户选择文件 → uploadFiles() → 得到 upload_id + html_files
                    ↓
用户点击保存 → updatePage(id, { title, ..., upload_id, entry_file })
                    ↓
后端收到 upload_id → 删除旧文件 → 复制新文件 → 更新 DB → 返回
```

## 文件范围

- `src/models/page.rs` — UpdatePageParams + update_page SQL
- `src/api/pages.rs` — UpdatePageRequest + update_page_handler 文件替换逻辑
- `web/src/api/pages.ts` — updatePage 参数
- `web/src/components/PageEditModal.tsx` — 上传 UI
