import { useState } from 'react';
import { Menu, X } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

const pub = (file) => `${import.meta.env.BASE_URL}${file}`;

const defaultNav = [
  { id: 'home', label: 'GŁÓWNA' },
  { id: 'mods', label: 'MODY' },
  { id: 'settings', label: 'USTAWIENIA' },
];

export default function SteampunkNavbar({ activeTab, onSelectTab, navItems = defaultNav }) {
  const [mobileOpen, setMobileOpen] = useState(false);

  return (
    <nav className="fixed left-0 right-0 top-8 z-40 border-b border-stone-light/40 bg-background/90 backdrop-blur-md">
      <div className="mx-auto flex h-14 max-w-7xl items-center justify-between px-4 lg:px-8">
        <button
          type="button"
          onClick={() => onSelectTab?.('home')}
          className="group flex min-w-0 shrink-0 items-center gap-3"
        >
          <img
            src={pub('mainlogo.png')}
            alt=""
            width={40}
            height={40}
            className="h-9 w-9 shrink-0 object-contain img-crisp"
          />
          <span className="font-mc text-[10px] leading-tight tracking-wide text-foreground sm:text-[11px]">
            CREATE <span className="text-primary">CRAFT</span>
          </span>
        </button>

        <div className="absolute left-1/2 hidden max-w-[50%] -translate-x-1/2 items-center gap-5 md:flex">
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => {
                onSelectTab?.(item.id);
                setMobileOpen(false);
              }}
              className={`shrink-0 font-mc text-[8px] tracking-widest transition-colors ${
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
            className="overflow-hidden border-b border-stone-light/40 bg-background/95 backdrop-blur-md md:hidden"
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
                  className={`text-left font-mc text-[9px] tracking-widest transition-colors ${
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
