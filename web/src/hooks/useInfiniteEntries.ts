import { useInfiniteQuery } from '@tanstack/react-query';
import { listEntries, type ListParams } from '../api/entries';

interface UseInfiniteEntriesOptions extends Omit<ListParams, 'cursor'> {
  enabled?: boolean;
}

export function useInfiniteEntries(options: UseInfiniteEntriesOptions = {}) {
  const { enabled = true, ...baseParams } = options;

  return useInfiniteQuery({
    queryKey: ['entries-infinite', baseParams],
    queryFn: ({ pageParam }) => listEntries({ ...baseParams, cursor: pageParam }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
    getPreviousPageParam: () => undefined,
    enabled,
    staleTime: 1000 * 60, // 1 minute
  });
}
