import { useState, useRef, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '../api/client';
import { fetchTagStats, renameTag, deleteTag } from '../api/tags';
import { listRules, createRule, updateRule, deleteRule, type TaggingRule, type CreateTaggingRuleData } from '../api/taggingRules';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import TokensPanel from '../components/settings/TokensPanel';
import { Pencil, Trash2, Plus } from 'lucide-react';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import { toast } from 'sonner';

const FIELD_LABELS: Record<string, string> = {
  title: '标题',
  url: 'URL',
  domainName: '域名',
  language: '语言',
  readingTime: '阅读时间',
  contentType: '内容类型',
};

const OPERATOR_LABELS: Record<string, string> = {
  eq: '等于',
  neq: '不等于',
  contains: '包含',
  not_contains: '不包含',
  matches: '匹配正则',
  gt: '大于',
  lt: '小于',
};

const FIELDS = Object.keys(FIELD_LABELS);
const OPERATORS = Object.keys(OPERATOR_LABELS);

export default function SettingsPage() {
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importResult, setImportResult] = useState('');
  const [editingTagId, setEditingTagId] = useState<string | null>(null);
  const [editingLabel, setEditingLabel] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; label: string } | null>(null);
  const qc = useQueryClient();

  // Tagging rule form state
  const [showRuleForm, setShowRuleForm] = useState(false);
  const [editingRuleId, setEditingRuleId] = useState<string | null>(null);
  const [ruleField, setRuleField] = useState('title');
  const [ruleOperator, setRuleOperator] = useState('contains');
  const [ruleValue, setRuleValue] = useState('');
  const [ruleTags, setRuleTags] = useState('');
  const [ruleTagInput, setRuleTagInput] = useState('');
  const [showRuleTagSuggest, setShowRuleTagSuggest] = useState(false);
  const [deleteRuleTarget, setDeleteRuleTarget] = useState<{ id: string; rule: string } | null>(null);
  const ruleTagContainerRef = useRef<HTMLDivElement>(null);

  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  const { data: rules = [] } = useQuery({
    queryKey: ['tagging-rules'],
    queryFn: listRules,
  });

  // Close suggestions when clicking outside
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ruleTagContainerRef.current && !ruleTagContainerRef.current.contains(e.target as Node)) {
        setShowRuleTagSuggest(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const importWallabag = useMutation({
    mutationFn: async (file: File) => {
      const text = await file.text();
      const data = JSON.parse(text);
      const res = await api.post('/import/wallabag', data);
      return res.data;
    },
    onSuccess: (data) => setImportResult(`导入 ${data.imported} 篇，跳过 ${data.skipped} 篇`),
    onError: () => setImportResult('导入失败'),
  });

  const exportAll = useMutation({
    mutationFn: async () => {
      const res = await api.get('/export');
      const blob = new Blob([JSON.stringify(res.data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `lettura-export-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    },
  });

  const renameMutation = useMutation({
    mutationFn: ({ id, label }: { id: string; label: string }) => renameTag(id, label),
    onSuccess: () => {
      setEditingTagId(null);
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
      toast.success('标签已重命名');
    },
    onError: () => toast.error('重命名失败'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteTag(id),
    onSuccess: () => {
      setDeleteTarget(null);
      qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
      toast.success('标签已删除');
    },
    onError: () => toast.error('删除失败'),
  });

  const createRuleMutation = useMutation({
    mutationFn: (data: CreateTaggingRuleData) => createRule(data),
    onSuccess: () => {
      resetRuleForm();
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已创建');
    },
    onError: () => toast.error('创建规则失败'),
  });

  const updateRuleMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateTaggingRuleData> }) => updateRule(id, data),
    onSuccess: () => {
      resetRuleForm();
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已更新');
    },
    onError: () => toast.error('更新规则失败'),
  });

  const deleteRuleMutation = useMutation({
    mutationFn: (id: string) => deleteRule(id),
    onSuccess: () => {
      setDeleteRuleTarget(null);
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已删除');
    },
    onError: () => toast.error('删除规则失败'),
  });

  const handleRenameKeyDown = (e: React.KeyboardEvent, tagId: string) => {
    if (e.key === 'Enter' && editingLabel.trim()) {
      renameMutation.mutate({ id: tagId, label: editingLabel.trim() });
    } else if (e.key === 'Escape') {
      setEditingTagId(null);
    }
  };

  const resetRuleForm = () => {
    setShowRuleForm(false);
    setEditingRuleId(null);
    setRuleField('title');
    setRuleOperator('contains');
    setRuleValue('');
    setRuleTags('');
    setRuleTagInput('');
  };

  const handleEditRule = (rule: TaggingRule) => {
    setEditingRuleId(rule.id);
    setRuleField(rule.rule.field);
    setRuleOperator(rule.rule.operator);
    setRuleValue(rule.rule.value);
    setRuleTags(rule.tags.join(', '));
    setShowRuleForm(true);
  };

  const handleSaveRule = (e: React.FormEvent) => {
    e.preventDefault();
    const tags = ruleTags.split(',').map((t) => t.trim()).filter(Boolean);
    if (!ruleValue.trim() || tags.length === 0) return;

    const data: CreateTaggingRuleData = {
      rule: { field: ruleField, operator: ruleOperator, value: ruleValue.trim() },
      tags,
    };

    if (editingRuleId) {
      updateRuleMutation.mutate({ id: editingRuleId, data });
    } else {
      createRuleMutation.mutate(data);
    }
  };

  const handleRuleTagSelect = (label: string) => {
    const existing = ruleTags.split(',').map((t) => t.trim()).filter(Boolean);
    if (!existing.includes(label)) {
      const newTags = existing.length > 0 ? [...existing, label].join(', ') : label;
      setRuleTags(newTags);
    }
    setRuleTagInput('');
    setShowRuleTagSuggest(false);
  };

  const ruleTagSuggestions = tagStats
    .filter((t) =>
      t.label.toLowerCase().includes(ruleTagInput.toLowerCase())
    )
    .filter((t) => {
      const existing = ruleTags.split(',').map((s) => s.trim().toLowerCase());
      return !existing.includes(t.label.toLowerCase());
    })
    .slice(0, 10);

  const formatRuleSummary = (rule: TaggingRule) => {
    const fieldLabel = FIELD_LABELS[rule.rule.field] || rule.rule.field;
    const opLabel = OPERATOR_LABELS[rule.rule.operator] || rule.rule.operator;
    return `${fieldLabel} ${opLabel} "${rule.rule.value}"`;
  };

  return (
    <div className="max-w-2xl">
      <h2 className="text-xl font-semibold mb-6 text-foreground">设置</h2>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">标签管理</h3>
        {tagStats.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无标签</p>
        ) : (
          <div className="border border-border rounded-lg overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-muted/50">
                  <th className="text-left px-4 py-2 font-medium">标签名</th>
                  <th className="text-right px-4 py-2 font-medium">文章数</th>
                  <th className="text-right px-4 py-2 font-medium">操作</th>
                </tr>
              </thead>
              <tbody>
                {tagStats.map((tag) => (
                  <tr key={tag.id} className="border-b border-border last:border-b-0">
                    <td className="px-4 py-2">
                      {editingTagId === tag.id ? (
                        <Input
                          value={editingLabel}
                          onChange={(e) => setEditingLabel(e.target.value)}
                          onKeyDown={(e) => handleRenameKeyDown(e, tag.id)}
                          onBlur={() => setEditingTagId(null)}
                          className="h-7 text-sm"
                          autoFocus
                        />
                      ) : (
                        tag.label
                      )}
                    </td>
                    <td className="text-right px-4 py-2 text-muted-foreground">{tag.entry_count}</td>
                    <td className="text-right px-4 py-2">
                      <div className="flex items-center justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0"
                          onClick={() => {
                            setEditingTagId(tag.id);
                            setEditingLabel(tag.label);
                          }}
                        >
                          <Pencil size={14} />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-7 w-7 p-0 hover:text-destructive"
                          onClick={() => setDeleteTarget({ id: tag.id, label: tag.label })}
                        >
                          <Trash2 size={14} />
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* Tag management delete confirmation */}
      <AlertDialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除标签</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除标签「{deleteTarget?.label}」吗？此操作将从所有文章中移除该标签，且不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Tagging rules section */}
      <section className="mb-8">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-medium text-foreground">标签规则</h3>
          {!showRuleForm && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                resetRuleForm();
                setShowRuleForm(true);
              }}
            >
              <Plus size={14} className="mr-1" /> 新增规则
            </Button>
          )}
        </div>

        {showRuleForm && (
          <form onSubmit={handleSaveRule} className="border border-border rounded-lg p-4 mb-4 space-y-3">
            <div className="flex items-center gap-2 flex-wrap">
              <select
                value={ruleField}
                onChange={(e) => setRuleField(e.target.value)}
                className="h-8 rounded-md border border-input bg-background px-2 text-sm"
              >
                {FIELDS.map((f) => (
                  <option key={f} value={f}>{FIELD_LABELS[f]}</option>
                ))}
              </select>
              <select
                value={ruleOperator}
                onChange={(e) => setRuleOperator(e.target.value)}
                className="h-8 rounded-md border border-input bg-background px-2 text-sm"
              >
                {OPERATORS.map((op) => (
                  <option key={op} value={op}>{OPERATOR_LABELS[op]}</option>
                ))}
              </select>
              <Input
                value={ruleValue}
                onChange={(e) => setRuleValue(e.target.value)}
                placeholder="值"
                className="h-8 text-sm flex-1 min-w-[120px]"
              />
            </div>
            <div ref={ruleTagContainerRef} className="relative">
              <div className="flex items-center gap-2">
                <Input
                  value={ruleTags}
                  onChange={(e) => setRuleTags(e.target.value)}
                  placeholder="标签（逗号分隔）"
                  className="h-8 text-sm flex-1"
                />
                <Input
                  value={ruleTagInput}
                  onChange={(e) => {
                    setRuleTagInput(e.target.value);
                    setShowRuleTagSuggest(true);
                  }}
                  onFocus={() => setShowRuleTagSuggest(true)}
                  placeholder="搜索标签..."
                  className="h-8 text-sm w-36"
                />
              </div>
              {showRuleTagSuggest && ruleTagInput && ruleTagSuggestions.length > 0 && (
                <div className="absolute z-50 w-full mt-1">
                  <Command className="border border-border shadow-md">
                    <CommandList>
                      <CommandEmpty>无匹配</CommandEmpty>
                      <CommandGroup>
                        {ruleTagSuggestions.map((tag) => (
                          <CommandItem
                            key={tag.id}
                            value={tag.label}
                            onSelect={() => handleRuleTagSelect(tag.label)}
                          >
                            {tag.label}
                          </CommandItem>
                        ))}
                      </CommandGroup>
                    </CommandList>
                  </Command>
                </div>
              )}
            </div>
            <div className="flex items-center gap-2">
              <Button type="submit" size="sm" disabled={!ruleValue.trim() || !ruleTags.trim()}>
                {editingRuleId ? '更新规则' : '创建规则'}
              </Button>
              <Button type="button" size="sm" variant="ghost" onClick={resetRuleForm}>
                取消
              </Button>
            </div>
          </form>
        )}

        {rules.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无标签规则</p>
        ) : (
          <div className="space-y-2">
            {rules.map((rule) => (
              <div
                key={rule.id}
                className="flex items-center justify-between border border-border rounded-lg px-4 py-2"
              >
                <div className="flex-1 min-w-0">
                  <span className="text-sm">{formatRuleSummary(rule)}</span>
                  <span className="text-sm text-muted-foreground ml-2">
                    → {rule.tags.join(', ')}
                  </span>
                </div>
                <div className="flex items-center gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0"
                    onClick={() => handleEditRule(rule)}
                  >
                    <Pencil size={14} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0 hover:text-destructive"
                    onClick={() => setDeleteRuleTarget({ id: rule.id, rule: formatRuleSummary(rule) })}
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Rule delete confirmation */}
      <AlertDialog open={!!deleteRuleTarget} onOpenChange={(open) => { if (!open) setDeleteRuleTarget(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除规则</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除规则「{deleteRuleTarget?.rule}」吗？此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => deleteRuleTarget && deleteRuleMutation.mutate(deleteRuleTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">导入</h3>
        <div className="space-y-2">
          <label className="text-sm text-muted-foreground block mb-1">Wallabag JSON 导入</label>
          <div className="flex items-center gap-2">
            <Input
              type="file"
              accept=".json"
              onChange={(e) => setImportFile(e.target.files?.[0] ?? null)}
              className="text-sm"
            />
            <Button
              onClick={() => importFile && importWallabag.mutate(importFile)}
              disabled={!importFile || importWallabag.isPending}
            >
              {importWallabag.isPending ? '导入中...' : '导入'}
            </Button>
          </div>
          {importResult && <p className="text-sm text-green-600 dark:text-green-400 mt-1">{importResult}</p>}
        </div>
      </section>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">导出</h3>
        <Button
          onClick={() => exportAll.mutate()}
          disabled={exportAll.isPending}
          variant="outline"
        >
          {exportAll.isPending ? '导出中...' : '导出全部数据 (JSON)'}
        </Button>
      </section>

      <section className="mb-8">
        <h3 className="font-medium mb-3 text-foreground">API 令牌</h3>
        <p className="text-sm text-muted-foreground mb-4">
          管理用于 lettura-cli 或其他第三方客户端访问你数据的个人访问令牌。
        </p>
        <TokensPanel />
      </section>
    </div>
  );
}
