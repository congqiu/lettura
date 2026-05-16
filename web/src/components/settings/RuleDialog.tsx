import { useState, useRef, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Command, CommandEmpty, CommandGroup, CommandItem, CommandList,
} from '@/components/ui/command';
import { toast } from 'sonner';
import { createRule, updateRule } from '@/api/taggingRules';
import { fetchTagStats } from '@/api/tags';
import type { TaggingRule, CreateTaggingRuleData } from '@/api/taggingRules';

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

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialData?: TaggingRule | null;
}

export default function RuleDialog({ open, onOpenChange, initialData }: Props) {
  const qc = useQueryClient();
  const isEdit = !!initialData;
  const [ruleField, setRuleField] = useState('title');
  const [ruleOperator, setRuleOperator] = useState('contains');
  const [ruleValue, setRuleValue] = useState('');
  const [ruleTags, setRuleTags] = useState('');
  const [ruleTagInput, setRuleTagInput] = useState('');
  const [showRuleTagSuggest, setShowRuleTagSuggest] = useState(false);
  const ruleTagContainerRef = useRef<HTMLDivElement>(null);

  const { data: tagStats = [] } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  useEffect(() => {
    if (open) {
      if (initialData) {
        setRuleField(initialData.rule.field);
        setRuleOperator(initialData.rule.operator);
        setRuleValue(initialData.rule.value);
        setRuleTags(initialData.tags.join(', '));
      } else {
        setRuleField('title');
        setRuleOperator('contains');
        setRuleValue('');
        setRuleTags('');
      }
      setRuleTagInput('');
      setShowRuleTagSuggest(false);
    }
  }, [open, initialData]);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ruleTagContainerRef.current && !ruleTagContainerRef.current.contains(e.target as Node)) {
        setShowRuleTagSuggest(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const createRuleMutation = useMutation({
    mutationFn: (data: CreateTaggingRuleData) => createRule(data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已创建');
      onOpenChange(false);
    },
    onError: () => toast.error('创建规则失败'),
  });

  const updateRuleMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<CreateTaggingRuleData> }) => updateRule(id, data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tagging-rules'] });
      toast.success('规则已更新');
      onOpenChange(false);
    },
    onError: () => toast.error('更新规则失败'),
  });

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    const tags = ruleTags.split(',').map((t) => t.trim()).filter(Boolean);
    if (!ruleValue.trim() || tags.length === 0) return;

    const data: CreateTaggingRuleData = {
      rule: { field: ruleField, operator: ruleOperator, value: ruleValue.trim() },
      tags,
    };

    if (isEdit && initialData) {
      updateRuleMutation.mutate({ id: initialData.id, data });
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
    .filter((t) => t.label.toLowerCase().includes(ruleTagInput.toLowerCase()))
    .filter((t) => {
      const existing = ruleTags.split(',').map((s) => s.trim().toLowerCase());
      return !existing.includes(t.label.toLowerCase());
    })
    .slice(0, 10);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg rounded-2xl">
        <DialogHeader>
          <DialogTitle>{isEdit ? '编辑标签规则' : '新增标签规则'}</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSave} className="space-y-4 py-2">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">匹配条件</label>
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
          </div>

          <div className="space-y-1.5" ref={ruleTagContainerRef}>
            <label className="text-sm font-medium">标签</label>
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
              <div className="relative z-50 mt-1">
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

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)} className="rounded-lg">
              取消
            </Button>
            <Button
              type="submit"
              disabled={!ruleValue.trim() || !ruleTags.trim() || createRuleMutation.isPending || updateRuleMutation.isPending}
              className="rounded-lg"
            >
              {createRuleMutation.isPending || updateRuleMutation.isPending
                ? '保存中...'
                : isEdit ? '更新规则' : '创建规则'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
