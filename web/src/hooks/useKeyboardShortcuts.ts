import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';

interface ShortcutHandlers {
  onStar?: () => void;
  onArchive?: () => void;
  onDelete?: () => void;
  onEdit?: () => void;
  onBack?: () => void;
}

export function useKeyboardShortcuts(handlers: ShortcutHandlers = {}) {
  const navigate = useNavigate();
  const handlersRef = useRef(handlers);
  // Sync the latest handlers into the ref after each render so the keydown
  // listener (registered once below) always sees fresh callbacks without
  // re-binding the event listener.
  useEffect(() => {
    handlersRef.current = handlers;
  });

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement).isContentEditable) {
        return;
      }
      const h = handlersRef.current;
      switch (e.key) {
        case 's': e.preventDefault(); h.onStar?.(); break;
        case 'a': e.preventDefault(); h.onArchive?.(); break;
        case 'e': e.preventDefault(); h.onEdit?.(); break;
        case 'Backspace':
        case 'h':
          if (!e.metaKey && !e.ctrlKey) {
            if (h.onBack) {
              h.onBack();
            } else {
              navigate(-1);
            }
          }
          break;
        case '1': navigate('/'); break;
        case '2': navigate('/archived'); break;
        case '3': navigate('/starred'); break;
        case '4': navigate('/memos'); break;
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [navigate]);
}

export function useListKeyboardNav(
  entries: { id: string }[],
  selectedIndex: number,
  setSelectedIndex: (i: number) => void,
) {
  const navigate = useNavigate();
  const entriesRef = useRef(entries);
  const selectedRef = useRef(selectedIndex);
  // Sync latest values into refs after render — see comment above.
  useEffect(() => {
    entriesRef.current = entries;
    selectedRef.current = selectedIndex;
  });

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;
      const currentEntries = entriesRef.current;
      const currentSelected = selectedRef.current;
      switch (e.key) {
        case 'j':
          e.preventDefault();
          setSelectedIndex(Math.min(currentSelected + 1, currentEntries.length - 1));
          break;
        case 'k':
          e.preventDefault();
          setSelectedIndex(Math.max(currentSelected - 1, 0));
          break;
        case 'Enter':
        case 'o':
          if (currentEntries[currentSelected]) {
            const entryIds = currentEntries.map(e => e.id);
            navigate(`/entry/${currentEntries[currentSelected].id}`, {
              state: { entryIds, currentIndex: currentSelected },
            });
          }
          break;
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [setSelectedIndex, navigate]);
}
