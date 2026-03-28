import { useEffect } from 'react';
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

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore when typing in inputs
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement).isContentEditable) {
        return;
      }

      switch (e.key) {
        case 's':
          e.preventDefault();
          handlers.onStar?.();
          break;
        case 'a':
          e.preventDefault();
          handlers.onArchive?.();
          break;
        case 'e':
          e.preventDefault();
          handlers.onEdit?.();
          break;
        case 'Backspace':
        case 'h':
          if (!e.metaKey && !e.ctrlKey) {
            handlers.onBack?.() || navigate(-1);
          }
          break;
        case 'g':
          // g then key combos
          break;
        case '1':
          navigate('/');
          break;
        case '2':
          navigate('/archived');
          break;
        case '3':
          navigate('/starred');
          break;
        case '4':
          navigate('/memos');
          break;
        case '?':
          // Could show shortcuts help modal
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [navigate, handlers]);
}

export function useListKeyboardNav(
  entries: { id: string }[],
  selectedIndex: number,
  setSelectedIndex: (i: number) => void,
) {
  const navigate = useNavigate();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;

      switch (e.key) {
        case 'j':
          e.preventDefault();
          setSelectedIndex(Math.min(selectedIndex + 1, entries.length - 1));
          break;
        case 'k':
          e.preventDefault();
          setSelectedIndex(Math.max(selectedIndex - 1, 0));
          break;
        case 'Enter':
        case 'o':
          if (entries[selectedIndex]) {
            navigate(`/entry/${entries[selectedIndex].id}`);
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [entries, selectedIndex, setSelectedIndex, navigate]);
}
