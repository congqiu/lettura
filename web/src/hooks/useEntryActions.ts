import { useMutation, useQueryClient } from '@tanstack/react-query';
import { updateEntry, refetchEntry } from '../api/entries';
import { toast } from 'sonner';

interface EntryData {
  id: string;
  is_starred: boolean;
  is_archived: boolean;
}

export function useEntryActions(
  entryId: string,
  entry: { is_starred: boolean; is_archived: boolean },
) {
  const qc = useQueryClient();

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['entries-infinite'] });
    qc.invalidateQueries({ queryKey: ['entry', entryId] });
  };

  const toggleStar = useMutation({
    mutationFn: () => updateEntry(entryId, { is_starred: !entry.is_starred }),
    onMutate: async () => {
      await qc.cancelQueries({ queryKey: ['entries-infinite'] });

      const previousEntries = qc.getQueriesData({ queryKey: ['entries-infinite'] });

      qc.setQueriesData<{
        pages: { entries: EntryData[]; next_cursor?: string | null }[];
        pageParams: (string | undefined)[];
      }>(
        { queryKey: ['entries-infinite'] },
        (old) => {
          if (!old) return old;
          return {
            ...old,
            pages: old.pages.map((page) => ({
              ...page,
              entries: page.entries.map((e) =>
                e.id === entryId ? { ...e, is_starred: !e.is_starred } : e
              ),
            })),
          };
        }
      );

      qc.setQueryData<EntryData>(['entry', entryId], (old) => {
        if (!old) return old;
        return { ...old, is_starred: !old.is_starred };
      });

      return { previousEntries, previousStarred: entry.is_starred };
    },
    onError: (_err, _vars, context) => {
      context?.previousEntries?.forEach(([queryKey, data]) => {
        qc.setQueryData(queryKey, data);
      });
      toast.error('操作失败，请重试');
    },
    onSuccess: (_data, _vars, context) => {
      invalidate();
      toast.success(context?.previousStarred ? '已取消收藏' : '已收藏');
    },
  });

  const toggleArchive = useMutation({
    mutationFn: () => updateEntry(entryId, { is_archived: !entry.is_archived }),
    onMutate: async () => {
      await qc.cancelQueries({ queryKey: ['entries-infinite'] });

      const previousEntries = qc.getQueriesData({ queryKey: ['entries-infinite'] });

      qc.setQueriesData<{
        pages: { entries: EntryData[]; next_cursor?: string | null }[];
        pageParams: (string | undefined)[];
      }>(
        { queryKey: ['entries-infinite'] },
        (old) => {
          if (!old) return old;
          return {
            ...old,
            pages: old.pages.map((page) => ({
              ...page,
              entries: page.entries.map((e) =>
                e.id === entryId ? { ...e, is_archived: !e.is_archived } : e
              ),
            })),
          };
        }
      );

      qc.setQueryData<EntryData>(['entry', entryId], (old) => {
        if (!old) return old;
        return { ...old, is_archived: !old.is_archived };
      });

      return { previousEntries, previousArchived: entry.is_archived };
    },
    onError: (_err, _vars, context) => {
      context?.previousEntries?.forEach(([queryKey, data]) => {
        qc.setQueryData(queryKey, data);
      });
      toast.error('操作失败，请重试');
    },
    onSuccess: (_data, _vars, context) => {
      invalidate();
      toast.success(context?.previousArchived ? '已取消归档' : '已归档');
    },
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
