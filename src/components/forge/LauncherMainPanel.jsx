import { motion } from 'framer-motion';
import { Play, CheckCircle2, Loader2 } from 'lucide-react';
import CogwheelFrame from './CogwheelFrame';

const HERO_BG = `${import.meta.env.BASE_URL}hero-bg-ObJtS6DH.jpg`;

export default function LauncherMainPanel({ user, connectionState, progress, statusText, onPlay }) {
  return (
    <section className="relative min-h-[calc(100vh-8rem)] overflow-hidden pb-12 pt-6">
      <motion.div
        className="absolute inset-0 z-0"
        animate={{ scale: [1, 1.02, 1] }}
        transition={{ duration: 24, repeat: Infinity, ease: 'easeInOut' }}
      >
        <img src={HERO_BG} alt="" className="h-full w-full object-cover" width={1920} height={1080} />
      </motion.div>
      <div className="absolute inset-0 z-[1] bg-gradient-to-r from-background/90 via-background/55 to-background/75" />
      <div className="absolute inset-0 z-[1] bg-gradient-to-t from-background via-background/35 to-transparent" />
      <div className="absolute inset-0 z-[1] bg-background/15" />

      <div className="relative z-10 mx-auto flex max-w-2xl justify-center px-4 pb-8 pt-12 lg:px-8">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.7, delay: 0.05 }}
          className="w-full"
        >
          <CogwheelFrame>
            <div className="p-6 lg:p-8">
              <p className="mb-3 text-xs font-semibold tracking-[0.2em] text-brass-light">
                NEOFORGE · CREATECRAFTS
              </p>
              <h1 className="mb-4 text-3xl font-black leading-[0.95] text-foreground lg:text-4xl xl:text-5xl">
                WITAJ,
                <br />
                <span className="text-gradient-emerald">{(user?.name || 'GRACZ').toUpperCase()}</span>
              </h1>
              <p className="mb-6 max-w-md text-sm leading-relaxed text-muted-foreground">
                Launcher zsynchronizuje mody, NeoForge i pliki gry. Uruchom grę, gdy jesteś gotów.
              </p>

              <button
                type="button"
                onClick={onPlay}
                disabled={connectionState !== 'idle'}
                className={`group relative inline-flex w-full items-center justify-center gap-3 overflow-hidden rounded-lg px-6 py-3.5 text-sm font-bold transition-all duration-300 sm:w-auto ${
                  connectionState === 'connected'
                    ? 'bg-primary text-primary-foreground glow-emerald'
                    : connectionState !== 'idle'
                      ? 'cursor-not-allowed bg-muted text-muted-foreground'
                      : 'bg-primary text-primary-foreground glow-emerald hover:scale-[1.02] hover:shadow-[0_0_50px_hsl(142_69%_58%_/_0.5)]'
                }`}
              >
                {connectionState === 'idle' && (
                  <>
                    <Play size={20} />
                    URUCHOM GRĘ
                  </>
                )}
                {connectionState === 'connected' && (
                  <>
                    <CheckCircle2 size={20} />
                    W GRZE
                  </>
                )}
                {connectionState !== 'idle' && connectionState !== 'connected' && (
                  <>
                    <Loader2 size={20} className="z-10 animate-spin text-primary-foreground" />
                    <span className="z-10">{progress.toFixed(0)}%</span>
                    <div
                      className="absolute bottom-0 left-0 top-0 bg-foreground/10 transition-all duration-200"
                      style={{ width: `${progress}%` }}
                    />
                  </>
                )}
              </button>
              <div className="mt-4 flex items-center gap-2 text-[11px] text-muted-foreground">
                <span
                  className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                    connectionState === 'idle' ? 'bg-primary' : 'animate-pulse-glow bg-secondary'
                  }`}
                />
                <span>{statusText}</span>
              </div>
            </div>
          </CogwheelFrame>
        </motion.div>
      </div>
    </section>
  );
}
