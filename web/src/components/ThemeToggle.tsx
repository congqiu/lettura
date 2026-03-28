import { useThemeStore } from '../store/theme';

export default function ThemeToggle() {
  const { theme, setTheme } = useThemeStore();

  const next = () => {
    const order = ['light', 'dark', 'system'] as const;
    const idx = order.indexOf(theme);
    setTheme(order[(idx + 1) % 3]);
  };

  const label = theme === 'light' ? '浅色' : theme === 'dark' ? '深色' : '跟随系统';

  return (
    <button
      onClick={next}
      className="px-2 py-1 text-xs rounded text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
      title={`主题: ${label}`}
    >
      {theme === 'dark' ? '🌙' : theme === 'light' ? '☀️' : '💻'} {label}
    </button>
  );
}
