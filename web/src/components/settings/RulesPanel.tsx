import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listRules, deleteRule, type TaggingRule } from '@/api/taggingRules';
import { Button } from '@/components/ui/button';
import { Plus, Pencil, Trash2, ListChecks } from 'lucide-react';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';
import RuleDialog from './RuleDialog';

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

function formatRuleSummary(rule: TaggingRule) {
  const fieldLabel = FIELD_LABELS[rule.rule.field] || rule.rule.field;
  const opLabel = OPERATOR_LABELS[rule.rule.operator] || rule.rule.operator;
  return `${fieldLabel} ${opLabel} "${rule.rule.value}"`;
}

export default function RulesPanel() {
  const qc = useQueryClient();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<TaggingRule | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; rule: string } | null>(null);

  const { data: rules = [] } = useQuery({
    queryKey: ['tagging-rules'],
    queryFn: listRules,
  });

  const deleteRuleMutation = useMutation({
    mutationFn: (id: string) => deleteRule(id),
    onSuccess: () => {
      setDeleteTarget(null);
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已删除');
    },
    onError: () => toast.error('删除规则失败'),
  });

  const handleAdd = () => {
    setEditingRule(null);
    setDialogOpen(true);
  };

  const handleEdit = (rule: TaggingRule) => {
    setEditingRule(rule);
    setDialogOpen(true);
  };

  return (
    <div className="animate-fade-in">
      <div className="flex items-center justify-between mb-5">
        <div>
          <h3 className="text-title font-semibold">标签规则</h3>
          <p className="text-sm text-muted-foreground mt-1">根据文章属性自动打标签</p>
        </div>
        <Button size="sm" variant="outline" onClick={handleAdd} className="h-8 rounded-lg">
          <Plus size={14} className="mr-1.5" /> 新增规则
        </Button>
      </div>

      {rules.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-14 text-center border border-dashed border-border/60 rounded-xl bg-muted/20">
          <div className="w-12 h-12 rounded-2xl bg-secondary flex items-center justify-center mb-3">
            <ListChecks size={22} className="text-muted-foreground/50" />
          </div>
          <p className="text-sm text-muted-foreground">暂无标签规则</p>
          <p className="text-xs text-muted-foreground/70 mt-1">创建规则来自动为文章添加标签</p>
          <Button size="sm" variant="outline" onClick={handleAdd} className="mt-4 rounded-lg h-8">
            <Plus size={14} className="mr-1.5" /> 新增规则
          </Button>
        </div>
      ) : (
        <div className="space-y-2">
          {rules.map((rule) => (
            <div
              key={rule.id}
              className="flex items-center justify-between border border-border/50 rounded-xl px-4 py-3 hover:bg-muted/20 transition-colors"
            >
              <div className="flex-1 min-w-0">
                <span className="text-sm font-medium">{formatRuleSummary(rule)}</span>
                <span className="text-sm text-muted-foreground ml-2">
                  → {rule.tags.join(', ')}
                </span>
              </div>
              <div className="flex items-center gap-0.5 shrink-0 ml-3">
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 rounded-lg"
                  onClick={() => handleEdit(rule)}
                >
                  <Pencil size={13} />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 rounded-lg hover:text-destructive"
                  onClick={() => setDeleteTarget({ id: rule.id, rule: formatRuleSummary(rule) })}
                >
                  <Trash2 size={13} />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      <RuleDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        initialData={editingRule}
      />

      <AlertDialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}>
        <AlertDialogContent className="rounded-2xl">
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除规则</AlertDialogTitle>
            <AlertDialogDescription>
              确定要删除规则「{deleteTarget?.rule}」吗？此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel className="rounded-lg">取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              className="rounded-lg"
              onClick={() => deleteTarget && deleteRuleMutation.mutate(deleteTarget.id)}
            >
              删除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
