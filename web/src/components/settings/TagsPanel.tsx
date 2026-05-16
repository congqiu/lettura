import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { fetchTagStats, renameTag, deleteTag } from '@/api/tags';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Pencil, Trash2, Tag, ArrowRight } from 'lucide-react';
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';
import { cn } from '@/lib/utils';

export default function TagsPanel() {
  const qc = useQueryClient();
  const [editingTagId, setEditingTagId] = useState<string | null>(null);
  const [editingLabel, setEditingLabel] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; label: string } | null>(null);

  const { data: tagStats = [], isLoading } = useQuery({
    queryKey: ['tags', 'stats'],
    queryFn: fetchTagStats,
  });

  const tagsWithEntries = tagStats.filter((t) => t.entry_count > 0);
  const tagsWithoutEntries = tagStats.filter((t) => t.entry_count === 0);

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

  const handleRenameKeyDown = (e: React.KeyboardEvent, tagId: string) => {
    if (e.key === 'Enter' && editingLabel.trim()) {
      renameMutation.mutate({ id: tagId, label: editingLabel.trim() });
    } else if (e.key === 'Escape') {
      setEditingTagId(null);
    }
  };

  function TagCard({ tag, variant = 'default' }: { tag: typeof tagStats[0]; variant?: 'default' | 'ghost' }) {
    const isEditing = editingTagId === tag.id;
    const isGhost = variant === 'ghost';

    return (
      <div
        className={cn(
          'group relative flex items-center gap-2 rounded-2xl border transition-all',
          isGhost
            ? 'pl-3 pr-2 py-1.5 bg-muted/30 border-border/20 hover:bg-muted/60 hover:border-border/50'
            : 'pl-4 pr-2 py-2 bg-card border-border/50 hover:border-primary/30 hover:bg-primary/[0.03] hover:shadow-sm'
        )}
      >
        {isEditing ? (
          <Input
            value={editingLabel}
            onChange={(e) => setEditingLabel(e.target.value)}
            onKeyDown={(e) => handleRenameKeyDown(e, tag.id)}
            onBlur={() => setEditingTagId(null)}
            className={cn(
              'h-6 text-sm px-1 py-0 border-0 bg-transparent focus-visible:ring-0 focus-visible:ring-offset-0',
              isGhost ? 'w-20' : 'w-28'
            )}
            autoFocus
          />
        ) : (
          <>
            <Link
              to={`/?tag=${encodeURIComponent(tag.label)}`}
              className={cn(
                'text-sm font-medium truncate transition-colors',
                isGhost ? 'text-muted-foreground hover:text-foreground max-w-[100px]' : 'text-foreground hover:text-primary max-w-[140px]'
              )}
            >
              {tag.label}
            </Link>
            <span className={cn('text-[11px] tabular-nums shrink-0', isGhost ? 'text-muted-foreground/60' : 'text-muted-foreground')}>
              {tag.entry_count}
            </span>
          </>
        )}

        {!isEditing && (
          <div className="flex items-center gap-0 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 rounded-md text-muted-foreground hover:text-foreground"
              onClick={(e) => {
                e.preventDefault();
                setEditingTagId(tag.id);
                setEditingLabel(tag.label);
              }}
            >
              <Pencil size={11} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 rounded-md text-muted-foreground hover:text-destructive"
              onClick={(e) => {
                e.preventDefault();
                setDeleteTarget({ id: tag.id, label: tag.label });
              }}
            >
              <Trash2 size={11} />
            </Button>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="animate-fade-in">
      <div className="flex items-center justify-between mb-5">
        <div>
          <h3 className="text-title font-semibold">标签管理</h3>
          <p className="text-sm text-muted-foreground mt-1">{tagStats.length} 个标签 · 点击标签查看文章</p>
        </div>
        <Link
          to="/?untagged=true"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-primary transition-colors"
        >
          未标签文章
          <ArrowRight size={14} />
        </Link>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12">
          <div className="w-5 h-5 border-2 border-muted border-t-primary rounded-full animate-spin" />
        </div>
      ) : tagStats.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-14 text-center border border-dashed border-border/60 rounded-xl bg-muted/20">
          <div className="w-12 h-12 rounded-2xl bg-secondary flex items-center justify-center mb-3">
            <Tag size={22} className="text-muted-foreground/50" />
          </div>
          <p className="text-sm text-muted-foreground">暂无标签</p>
          <p className="text-xs text-muted-foreground/70 mt-1">为文章添加标签后将在此管理</p>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Active tags */}
          {tagsWithEntries.length > 0 && (
            <div className="flex flex-wrap gap-3">
              {tagsWithEntries.map((tag) => (
                <TagCard key={tag.id} tag={tag} />
              ))}
            </div>
          )}

          {/* Unused tags */}
          {tagsWithoutEntries.length > 0 && (
            <div>
              <h4 className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/60 mb-3">
                无文章的标签
              </h4>
              <div className="flex flex-wrap gap-2">
                {tagsWithoutEntries.map((tag) => (
                  <TagCard key={tag.id} tag={tag} variant="ghost" />
                ))}
              </div>
            </div>
          )}
        </div>
      )}

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
    </div>
  );
}
