import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updateEntry, refetchEntry } from '../api/entries';
import { toast } from 'sonner';

export function useEntryActions(
  entryId: string,
  entry: { is_starred: boolean; is_archived: boolean },
) {
  const qc = useQueryClient();

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['entries'] });
    qc.invalidateQueries({ queryKey: ['entry', entryId] });
  };

  const toggleStar = useMutation({
    mutationFn: () => updateEntry(entryId, { is_starred: !entry.is_starred }),
    onSuccess: () => {
      invalidate();
      toast.success(entry.is_starred ? '已取消收藏' : '已收藏');
    },
    onError: () => toast.error('操作失败，请重试'),
  });

  const toggleArchive = useMutation({
    mutationFn: () => updateEntry(entryId, { is_archived: !entry.is_archived }),
    onSuccess: () => {
      invalidate();
      toast.success(entry.is_archived ? '已取消归档' : '已归档');
    },
    onError: () => toast.error('操作失败，请重试'),
  });

  const refetch = useMutation({
    mutationFn: () => refetchEntry(entryId),
    onSuccess: () => {
      invalidate();
      toast.success('已重新抓取');
    },
    onError: () => toast.error('重新抓取失败'),
  });

  return { toggleStar, toggleArchive, refetch };
}
