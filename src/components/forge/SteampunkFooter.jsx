const links = [
  { label: 'DISCORD', title: 'Create Crafts PL — Discord', href: 'https://discord.gg/JqG9u3aPMm' },
  { label: 'CREATECRAFTS.PL', title: 'createcrafts.pl', href: 'https://createcrafts.pl' },
  { label: 'REGULAMIN', title: 'Regulamin', href: 'https://createcrafts.pl/regulamin' },
  { label: 'KONTAKT', title: 'Kontakt — Discord', href: 'https://discord.gg/JqG9u3aPMm' },
];

async function openInBrowser(url, e) {
  e?.preventDefault();
  e?.stopPropagation();
  const open = window.electronAPI?.openExternalUrl;
  if (open) {
    try {
      const r = await open(url);
      if (r && !r.ok) console.warn('openExternalUrl:', r.error || url);
    } catch (err) {
      console.warn('openExternalUrl failed', err);
    }
    return;
  }
  // Poza Electronem (np. podgląd w przeglądarce) — w Electron NIE używaj window.open (otwiera drugie okno aplikacji).
  window.open(url, '_blank', 'noopener,noreferrer');
}

export default function SteampunkFooter() {
  return (
    <footer className="relative border-t border-glass-border bg-background">
      <div className="mx-auto max-w-7xl px-4 py-6 lg:px-8">
        <div className="flex flex-col items-center gap-4">
          <div className="flex flex-wrap items-center justify-center gap-6">
            {links.map((link) => (
              <button
                key={link.label}
                type="button"
                title={link.title}
                onClick={(e) => openInBrowser(link.href, e)}
                className="cursor-pointer border-none bg-transparent text-[11px] font-semibold tracking-[0.15em] text-muted-foreground transition-colors hover:text-foreground"
              >
                {link.label}
              </button>
            ))}
          </div>
          <p className="text-[10px] tracking-wider text-muted-foreground/50">Zbudowane z trybików i miłości</p>
        </div>
      </div>
    </footer>
  );
}
