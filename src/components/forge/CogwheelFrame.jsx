import { motion } from 'framer-motion';
import { Settings } from 'lucide-react';

export default function CogwheelFrame({ children, className = '' }) {
  return (
    <div className={`relative ${className}`}>
      <motion.div
        className="absolute -top-4 -left-4 z-10"
        animate={{ rotate: 360 }}
        transition={{ duration: 12, repeat: Infinity, ease: 'linear' }}
      >
        <Settings size={32} className="text-brass-light/60" strokeWidth={1.5} />
      </motion.div>
      <motion.div
        className="absolute -top-3 -right-3 z-10"
        animate={{ rotate: -360 }}
        transition={{ duration: 10, repeat: Infinity, ease: 'linear' }}
      >
        <Settings size={24} className="text-brass/50" strokeWidth={1.5} />
      </motion.div>
      <motion.div
        className="absolute -bottom-4 -left-3 z-10"
        animate={{ rotate: -360 }}
        transition={{ duration: 14, repeat: Infinity, ease: 'linear' }}
      >
        <Settings size={28} className="text-brass-light/40" strokeWidth={1.5} />
      </motion.div>
      <motion.div
        className="absolute -bottom-3 -right-4 z-10"
        animate={{ rotate: 360 }}
        transition={{ duration: 9, repeat: Infinity, ease: 'linear' }}
      >
        <Settings size={30} className="text-brass/50" strokeWidth={1.5} />
      </motion.div>

      <div className="absolute inset-0 rounded-xl border border-brass-dim/30" />
      <div className="absolute inset-[2px] rounded-xl border border-brass-dim/20" />

      <div className="relative overflow-hidden rounded-xl border border-glass-border bg-background/80 backdrop-blur-md">
        {children}
      </div>
    </div>
  );
}
