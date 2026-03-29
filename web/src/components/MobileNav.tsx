import { NavLink } from 'react-router-dom';
import { Menu, X } from 'lucide-react';

const links = [
  { to: '/', label: '未读', end: true },
  { to: '/archived', label: '归档', end: false },
  { to: '/starred', label: '收藏', end: false },
  { to: '/memos', label: '收集箱', end: false },
  { to: '/pages', label: '展示', end: false },
];

// Hamburger button only — drawer is rendered in Layout to escape header stacking
export function MobileNavButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="md:hidden p-2 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors"
      aria-label="打开菜单"
    >
      <Menu size={20} />
    </button>
  );
}

export function MobileDrawer({ open, onClose }: { open: boolean; onClose: () => void }) {
  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `block px-4 py-3 rounded-lg text-sm transition-colors ${
      isActive
        ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
    }`;

  return (
    <>
      {/* Overlay */}
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/40 dark:bg-black/60 md:hidden"
          onClick={onClose}
        />
      )}

      {/* Drawer — rendered at root level so it sits below header */}
      <div
        className={`fixed bottom-0 left-0 right-0 z-50 bg-white dark:bg-gray-900 rounded-t-2xl shadow-2xl md:hidden transition-transform duration-300 ${
          open ? 'translate-y-0' : 'translate-y-full pointer-events-none'
        }`}
      >
        <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-800">
          <span className="font-bold text-base">Lettura</span>
          <button
            onClick={onClose}
            className="p-2 text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-full"
            aria-label="关闭"
          >
            <X size={20} />
          </button>
        </div>
        <nav className="p-3 pb-[calc(0.75rem+env(safe-area-inset-bottom))] space-y-1" onClick={onClose}>
          {links.map((link) => (
            <NavLink key={link.to} to={link.to} end={link.end} className={linkClass}>
              {link.label}
            </NavLink>
          ))}
        </nav>
      </div>
    </>
  );
}

// Desktop tabs — shown on md+
export function DesktopNav() {
  return (
    <div className="hidden md:flex items-center gap-1">
      {links.map((link) => (
        <NavLink
          key={link.to}
          to={link.to}
          end={link.end}
          className={({ isActive }) =>
            `flex items-center gap-1.5 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive
                ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-900 dark:hover:text-gray-200'
            }`
          }
        >
          {link.label}
        </NavLink>
      ))}
    </div>
  );
}
