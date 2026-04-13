import { useState } from 'react';
import { Settings, Menu, X } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const defaultNav = [
  { id: 'home', label: 'GŁÓWNA' },
  { id: 'mods', label: 'MODY' },
  { id: 'settings', label: 'USTAWIENIA' },
];

export default function SteampunkNavbar({ activeTab, onSelectTab, navItems = defaultNav }) {
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <nav className="fixed left-0 right-0 top-8 z-40 border-b border-brass-dim/20 bg-background/60 backdrop-blur-xl">
      <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-4 lg:px-8">
        <button
          type="button"
          onClick={() => onSelectTab?.('home')}
          className="group flex shrink-0 items-center gap-2"
        >
          <motion.div animate={{ rotate: 360 }} transition={{ duration: 8, repeat: Infinity, ease: 'linear' }}>
            <Settings size={22} className="text-brass-light" />
          </motion.div>
          <span className="text-base font-black tracking-wider text-foreground">
            CREATE <span className="text-primary">CRAFT</span>
          </span>
        </button>

        <div className="absolute left-1/2 hidden -translate-x-1/2 items-center gap-6 md:flex">
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => {
                onSelectTab?.(item.id);
                setMobileOpen(false);
              }}
              className={`text-xs font-semibold tracking-[0.15em] transition-colors ${
                activeTab === item.id ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="flex w-10 shrink-0 justify-end md:hidden">
          <button
            type="button"
            className="text-muted-foreground"
            onClick={() => setMobileOpen(!mobileOpen)}
            aria-label="Menu"
          >
            {mobileOpen ? <X size={20} /> : <Menu size={20} />}
          </button>
        </div>
      </div>

      <AnimatePresence>
        {mobileOpen && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden border-b border-glass-border bg-background/95 backdrop-blur-xl md:hidden"
          >
            <div className="flex flex-col gap-3 px-6 py-4">
              {navItems.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => {
                    onSelectTab?.(item.id);
                    setMobileOpen(false);
                  }}
                  className={`text-left text-sm font-semibold tracking-widest transition-colors ${
                    activeTab === item.id ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </nav>
  );
}
