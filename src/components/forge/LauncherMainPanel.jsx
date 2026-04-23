import { motion } from 'framer-motion';
import { Play, CheckCircle2, Loader2 } from 'lucide-react';
import CogwheelFrame from './CogwheelFrame';

const HERO_BG = `${import.meta.env.BASE_URL}hero-mc.jpg`;

export default function LauncherMainPanel({ user, connectionState, progress, statusText, onPlay }) {
  return (
    <section className="relative min-h-[calc(100vh-8rem)] overflow-hidden pb-12 pt-6">
      <div className="absolute inset-0 z-0">
        <img
          src={HERO_BG}
          alt=""
          className="h-full w-full min-h-[420px] object-cover pixelated opacity-60 img-crisp lg:min-h-0"
          width={1920}
          height={1080}
        />
        <div className="absolute inset-0 bg-gradient-to-r from-background via-background/85 to-background/30" />
        <div className="absolute inset-0 bg-gradient-to-b from-background/40 via-transparent to-background" />
        <div className="absolute inset-0 bg-grid opacity-30" />
      </div>

      <div className="relative z-10 mx-auto flex max-w-2xl justify-center px-4 pb-8 pt-10 lg:px-8">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.55 }}
          className="w-full max-w-xl"
        >
          <CogwheelFrame>
            <div className="p-6 lg:p-8">
              <p className="mb-3 font-mc text-[8px] tracking-[0.2em] text-brass-light">NEOFORGE / CREATECRAFTS</p>
              <h1 className="mb-3 font-mc text-lg leading-snug text-foreground sm:text-xl lg:text-2xl">
                WITAJ,
                <br />
                <span className="text-gradient-emerald">{(user?.name || 'GRACZ').toUpperCase()}</span>
              </h1>
              <p className="mb-6 max-w-md font-pixel text-lg leading-snug text-muted-foreground">
                Mody i NeoForge synchronizują się z serwerem. Naciśnij Graj, gdy będziesz gotowy.
              </p>

              <button
                type="button"
                onClick={onPlay}
                disabled={connectionState !== 'idle'}
                className={`group relative inline-flex w-full items-center justify-center gap-3 px-6 py-4 transition-all duration-300 sm:w-auto ${
                  connectionState === 'connected'
                    ? 'btn-mc btn-mc-primary glow-emerald'
                    : connectionState !== 'idle'
                      ? 'btn-mc cursor-not-allowed opacity-70'
                      : 'btn-mc btn-mc-primary'
                }`}
              >
                {connectionState === 'idle' && (
                  <>
                    <Play size={18} className="img-crisp" />
                    GRAJ
                  </>
                )}
                {connectionState === 'connected' && (
                  <>
                    <CheckCircle2 size={18} className="img-crisp" />
                    W GRZE
                  </>
                )}
                {connectionState !== 'idle' && connectionState !== 'connected' && (
                  <>
                    <Loader2 size={18} className="z-10 animate-spin text-primary-foreground img-crisp" />
                    <span className="z-10 font-mc text-[9px]">{progress.toFixed(0)}%</span>
                    <div
                      className="absolute bottom-0 left-0 top-0 bg-foreground/10 transition-all duration-200"
                      style={{ width: `${progress}%` }}
                    />
                  </>
                )}
              </button>
              <div className="mt-4 flex min-h-[1.5rem] items-start gap-2 font-pixel text-base text-muted-foreground">
                <span
                  className={`mt-1.5 h-2 w-2 shrink-0 ${
                    connectionState === 'idle' ? 'bg-primary' : 'animate-pulse-glow bg-secondary'
                  }`}
                />
                <span className="min-w-0 leading-snug">{statusText}</span>
              </div>
            </div>
          </CogwheelFrame>
        </motion.div>
      </div>
    </section>
  );
}
