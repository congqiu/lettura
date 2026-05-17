import { useSearchParams } from 'react-router-dom';
import { useMemo } from 'react';
import type { ListParams } from '../api/entries';

interface Props {
  filter?: 'unread' | 'archived' | 'starred';
}

export function useEntryListFilters({ filter }: Props) {
  const [searchParams] = useSearchParams();

  const tagFilter = searchParams.get('tag') || '';
  const excludeTag = searchParams.get('exclude_tag') || '';
  const untagged = searchParams.get('untagged') === 'true';

  const params: Omit<ListParams, 'cursor'> = {};
  if (filter === 'archived') params.is_archived = true;
  if (filter === 'starred') params.is_starred = true;
  if (filter === 'unread') params.is_archived = false;
  if (tagFilter) params.tag = tagFilter;
  if (excludeTag) params.exclude_tag = excludeTag;
  if (untagged) params.untagged = true;

  const titleKey = useMemo(() => {
    if (tagFilter) return 'tag';
    if (untagged) return 'untagged';
    if (excludeTag) return 'excludeTag';
    return filter || 'unread';
  }, [filter, tagFilter, untagged, excludeTag]);

  return { params, tagFilter, excludeTag, untagged, titleKey };
}