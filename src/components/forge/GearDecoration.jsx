import { Settings } from 'lucide-react';

export default function GearDecoration({ size = 120, className = '', reverse = false }) {
  return (
    <div className={`${reverse ? 'animate-gear-spin-reverse' : 'animate-gear-spin'} ${className}`}>
      <Settings size={size} className="text-muted-foreground/10" strokeWidth={1} />
    </div>
  );
}
