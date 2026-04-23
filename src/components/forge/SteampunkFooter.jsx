const links = [
  { label: 'DISCORD', title: 'Discord', href: 'https://discord.gg/JqG9u3aPMm' },
  { label: 'STRONA', title: 'createcrafts.pl', href: 'https://createcrafts.pl' },
  { label: 'REGULAMIN', title: 'Regulamin', href: 'https://createcrafts.pl/regulamin' },
  { label: 'KONTAKT', title: 'Discord', href: 'https://discord.gg/JqG9u3aPMm' },
];

async function openInBrowser(url, e) {
  e?.preventDefault();
  e?.stopPropagation();
  const open = window.launcher?.openExternalUrl;
  if (open) {
    try {
      const r = await open(url);
      if (r && !r.ok) console.warn('openExternalUrl:', r.error || url);
    } catch (err) {
      console.warn('openExternalUrl failed', err);
    }
    return;
  }
  window.open(url, '_blank', 'noopener,noreferrer');
}

export default function SteampunkFooter({ appVersion }) {
  return (
    <footer className="relative border-t border-stone-light/40 bg-background">
      <div className="mx-auto max-w-7xl px-4 py-5 lg:px-8">
        <div className="flex flex-col items-center gap-3">
          <div className="flex flex-wrap items-center justify-center gap-5">
            {links.map((link) => (
              <button
                key={link.label}
                type="button"
                title={link.title}
                onClick={(e) => openInBrowser(link.href, e)}
                className="cursor-pointer border-none bg-transparent font-mc text-[8px] tracking-[0.15em] text-muted-foreground transition-colors hover:text-foreground"
              >
                {link.label}
              </button>
            ))}
          </div>
          <p className="font-mc text-[7px] tracking-wider text-muted-foreground/70">
            {appVersion ? `v${appVersion}` : 'CREATECRAFTS'}
          </p>
        </div>
      </div>
    </footer>
  );
}
