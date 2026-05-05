import { useState, useRef, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '../api/client';
import { fetchTagStats, renameTag, deleteTag } from '../api/tags';
import { listRules, createRule, updateRule, deleteRule, type TaggingRule, type CreateTaggingRuleData } from '../api/taggingRules';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import TokensPanel from '../components/settings/TokensPanel';
import { Pencil, Trash2, Plus, Upload, Download, Settings2, Tag, FileJson, Shield } from 'lucide-react';
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

function SectionCard({
  icon: Icon,
  title,
  children,
  action,
}: {
  icon: React.ComponentType<{ size?: number; className?: string }>;
  title: string;
  children: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <section className="mb-8">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Icon size={17} className="text-muted-foreground/60" />
          <h3 className="font-semibold text-foreground text-[15px]">{title}</h3>
        </div>
        {action}
      </div>
      <div className="bg-card border border-border/60 rounded-xl p-5">
        {children}
      </div>
    </section>
  );
}

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
    <div className="max-w-2xl animate-fade-in">
      <div className="flex items-center gap-2.5 mb-6">
        <div className="w-9 h-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
          <Settings2 size={18} />
        </div>
        <h2 className="text-xl font-bold tracking-tight text-foreground">设置</h2>
      </div>

      {/* Tags management */}
      <SectionCard icon={Tag} title="标签管理">
        {tagStats.length === 0 ? (
          <p className="text-sm text-muted-foreground">暂无标签</p>
        ) : (
          <>
            {/* Desktop table */}
            <div className="border border-border/50 rounded-lg overflow-hidden hidden sm:block">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border/50 bg-muted/30">
                    <th className="text-left px-4 py-2.5 font-medium text-muted-foreground text-[13px]">标签名</th>
                    <th className="text-right px-4 py-2.5 font-medium text-muted-foreground text-[13px]">文章数</th>
                    <th className="text-right px-4 py-2.5 font-medium text-muted-foreground text-[13px]">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {tagStats.map((tag) => (
                    <tr key={tag.id} className="border-b border-border/40 last:border-b-0 hover:bg-muted/20 transition-colors">
                      <td className="px-4 py-2.5">
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
                          <span className="font-medium">{tag.label}</span>
                        )}
                      </td>
                      <td className="text-right px-4 py-2.5 text-muted-foreground tabular-nums">{tag.entry_count}</td>
                      <td className="text-right px-4 py-2.5">
                        <div className="flex items-center justify-end gap-0.5">
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 w-7 p-0 rounded-lg"
                            onClick={() => {
                              setEditingTagId(tag.id);
                              setEditingLabel(tag.label);
                            }}
                          >
                            <Pencil size={13} />
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="h-7 w-7 p-0 rounded-lg hover:text-destructive"
                            onClick={() => setDeleteTarget({ id: tag.id, label: tag.label })}
                          >
                            <Trash2 size={13} />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Mobile cards */}
            <div className="space-y-2 sm:hidden">
              {tagStats.map((tag) => (
                <div key={tag.id} className="border border-border/50 rounded-lg p-3.5 bg-card">
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
                    <span className="text-sm text-muted-foreground tabular-nums">{tag.entry_count} 篇</span>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 px-2 rounded-lg"
                      onClick={() => { setEditingTagId(tag.id); setEditingLabel(tag.label); }}
                    >
                      <Pencil size={13} className="mr-1.5" /> 编辑
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 px-2 rounded-lg hover:text-destructive"
                      onClick={() => setDeleteTarget({ id: tag.id, label: tag.label })}
                    >
                      <Trash2 size={13} className="mr-1.5" /> 删除
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}
      </SectionCard>

      {/* Tag delete confirmation */}
      <AlertDialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}>
        <AlertDialogContent className="rounded-2xl">
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除标签</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除标签「{deleteTarget?.label}」吗？此操作将从所有文章中移除该标签，且不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              className="rounded-lg"
              onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Tagging rules */}
      <SectionCard
        icon={Shield}
        title="标签规则"
        action={
          !showRuleForm && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                resetRuleForm();
                setShowRuleForm(true);
              }}
              className="h-8 rounded-lg"
            >
              <Plus size={14} className="mr-1.5" /> 新增规则
            </Button>
          )
        }
      >
        {showRuleForm && (
          <form onSubmit={handleSaveRule} className="border border-border/50 rounded-xl p-4 mb-4 space-y-3 bg-muted/20">
            <div className="flex items-center gap-2 flex-wrap">
              <select
                value={ruleField}
                onChange={(e) => setRuleField(e.target.value)}
                className="h-9 rounded-lg border border-input bg-background px-3 text-sm"
              >
                {FIELDS.map((f) => (
                  <option key={f} value={f}>{FIELD_LABELS[f]}</option>
                ))}
              </select>
              <select
                value={ruleOperator}
                onChange={(e) => setRuleOperator(e.target.value)}
                className="h-9 rounded-lg border border-input bg-background px-3 text-sm"
              >
                {OPERATORS.map((op) => (
                  <option key={op} value={op}>{OPERATOR_LABELS[op]}</option>
                ))}
              </select>
              <Input
                value={ruleValue}
                onChange={(e) => setRuleValue(e.target.value)}
                placeholder="值"
                className="h-9 text-sm flex-1 min-w-[120px] rounded-lg"
              />
            </div>
            <div ref={ruleTagContainerRef} className="relative">
              <div className="flex items-center gap-2">
                <Input
                  value={ruleTags}
                  onChange={(e) => setRuleTags(e.target.value)}
                  placeholder="标签（逗号分隔）"
                  className="h-9 text-sm flex-1 rounded-lg"
                />
                <Input
                  value={ruleTagInput}
                  onChange={(e) => {
                    setRuleTagInput(e.target.value);
                    setShowRuleTagSuggest(true);
                  }}
                  onFocus={() => setShowRuleTagSuggest(true)}
                  placeholder="搜索标签..."
                  className="h-9 text-sm w-36 rounded-lg"
                />
              </div>
              {showRuleTagSuggest && ruleTagInput && ruleTagSuggestions.length > 0 && (
                <div className="absolute z-50 w-full mt-1">
                  <Command className="border border-border/60 shadow-lg rounded-xl">
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
              <Button type="submit" size="sm" disabled={!ruleValue.trim() || !ruleTags.trim()} className="rounded-lg h-8">
                {editingRuleId ? '更新规则' : '创建规则'}
              </Button>
              <Button type="button" size="sm" variant="ghost" onClick={resetRuleForm} className="rounded-lg h-8">
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
                className="flex items-center justify-between border border-border/50 rounded-xl px-4 py-3 hover:bg-muted/20 transition-colors"
              >
                <div className="flex-1 min-w-0">
                  <span className="text-sm">{formatRuleSummary(rule)}</span>
                  <span className="text-sm text-muted-foreground ml-2">
                    → {rule.tags.join(', ')}
                  </span>
                </div>
                <div className="flex items-center gap-0.5 shrink-0 ml-3">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0 rounded-lg"
                    onClick={() => handleEditRule(rule)}
                  >
                    <Pencil size={13} />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 w-7 p-0 rounded-lg hover:text-destructive"
                    onClick={() => setDeleteRuleTarget({ id: rule.id, rule: formatRuleSummary(rule) })}
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </SectionCard>

      {/* Rule delete confirmation */}
      <AlertDialog open={!!deleteRuleTarget} onOpenChange={(open) => { if (!open) setDeleteRuleTarget(null); }}>
        <AlertDialogContent className="rounded-2xl">
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除规则</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除规则「{deleteRuleTarget?.rule}」吗？此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              className="rounded-lg"
              onClick={() => deleteRuleTarget && deleteRuleMutation.mutate(deleteRuleTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      {/* Import */}
      <SectionCard icon={Upload} title="导入">
        <div className="space-y-3">
          <label className="text-sm text-muted-foreground block">Wallabag JSON 导入</label>
          <div className="flex items-center gap-2">
            <Input
              type="file"
              accept=".json"
              onChange={(e) => setImportFile(e.target.files?.[0] ?? null)}
              className="text-sm rounded-lg h-9"
            />
            <Button
              onClick={() => importFile && importWallabag.mutate(importFile)}
              disabled={!importFile || importWallabag.isPending}
              className="rounded-lg h-9"
            >
              {importWallabag.isPending ? '导入中...' : '导入'}
            </Button>
          </div>
          {importResult && (
            <p className="text-sm text-success font-medium">{importResult}</p>
          )}
        </div>
      </SectionCard>

      {/* Export */}
      <SectionCard icon={Download} title="导出">
        <Button
          onClick={() => exportAll.mutate()}
          disabled={exportAll.isPending}
          variant="outline"
          className="rounded-lg h-9"
        >
          <FileJson size={15} className="mr-2" />
          {exportAll.isPending ? '导出中...' : '导出全部数据 (JSON)'}
        </Button>
      </SectionCard>

      {/* API Tokens */}
      <SectionCard icon={Shield} title="API 令牌">
        <p className="text-sm text-muted-foreground mb-4">
          管理用于 lettura-cli 或其他第三方客户端访问你数据的个人访问令牌。
        </p>
        <TokensPanel />
      </SectionCard>
    </div>
  );
}
