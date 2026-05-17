import { useState, useCallback } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';
import { bulkTagByIds, bulkUntagByIds, bulkDeleteByIds, bulkArchiveByIds } from '../api/tags';
import { toast } from 'sonner';

export function useEntrySelection() {
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [bulkTagInput, setBulkTagInput] = useState('');
  const [showBulkTagSuggest, setShowBulkTagSuggest] = useState(false);
  const [bulkUntagInput, setBulkUntagInput] = useState('');
  const [showBulkUntagSuggest, setShowBulkUntagSuggest] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const qc = useQueryClient();

  const toggleSelect = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
    setSelectionMode(false);
  }, []);

  const afterBulkOp = useCallback(() => {
    clearSelection();
    qc.invalidateQueries({ queryKey: ['entries-infinite'] });
    qc.invalidateQueries({ queryKey: ['tags', 'stats'] });
  }, [clearSelection, qc]);

  const bulkTagMutation = useMutation({
    mutationFn: ({ ids, tags }: { ids: string[]; tags: string[] }) => bulkTagByIds(ids, tags),
    onSuccess: () => {
      toast.success(`已为 ${selectedIds.size} 篇文章添加标签`);
      afterBulkOp();
    },
    onError: () => toast.error('批量打标签失败'),
  });

  const bulkUntagMutation = useMutation({
    mutationFn: ({ ids, tags }: { ids: string[]; tags: string[] }) => bulkUntagByIds(ids, tags),
    onSuccess: () => {
      toast.success(`已从 ${selectedIds.size} 篇文章移除标签`);
      afterBulkOp();
    },
    onError: () => toast.error('批量移除标签失败'),
  });

  const bulkArchiveMutation = useMutation({
    mutationFn: (ids: string[]) => bulkArchiveByIds(ids),
    onSuccess: () => {
      toast.success(`已归档 ${selectedIds.size} 篇文章`);
      afterBulkOp();
    },
    onError: () => toast.error('批量归档失败'),
  });

  const bulkDeleteMutation = useMutation({
    mutationFn: (ids: string[]) => bulkDeleteByIds(ids),
    onSuccess: () => {
      toast.success(`已删除 ${selectedIds.size} 篇文章`);
      afterBulkOp();
    },
    onError: () => toast.error('批量删除失败'),
  });

  const handleBulkTag = useCallback((label?: string) => {
    const tag = label || bulkTagInput.trim();
    if (!tag || selectedIds.size === 0) return;
    bulkTagMutation.mutate({ ids: Array.from(selectedIds), tags: [tag] });
    setBulkTagInput('');
    setShowBulkTagSuggest(false);
  }, [bulkTagInput, selectedIds, bulkTagMutation]);

  const handleBulkUntag = useCallback((label?: string) => {
    const tag = label || bulkUntagInput.trim();
    if (!tag || selectedIds.size === 0) return;
    bulkUntagMutation.mutate({ ids: Array.from(selectedIds), tags: [tag] });
    setBulkUntagInput('');
    setShowBulkUntagSuggest(false);
  }, [bulkUntagInput, selectedIds, bulkUntagMutation]);

  return {
    selectionMode, setSelectionMode,
    selectedIds, toggleSelect, clearSelection,
    bulkTagInput, setBulkTagInput,
    showBulkTagSuggest, setShowBulkTagSuggest,
    bulkUntagInput, setBulkUntagInput,
    showBulkUntagSuggest, setShowBulkUntagSuggest,
    deleteConfirmOpen, setDeleteConfirmOpen,
    bulkTagMutation, bulkUntagMutation,
    bulkArchiveMutation, bulkDeleteMutation,
    handleBulkTag, handleBulkUntag,
  };
}