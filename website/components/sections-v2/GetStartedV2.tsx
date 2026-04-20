import { InstallShButton } from '../InstallShButton';
import { EmailSignupForm } from '../EmailSignupForm';

interface GetStartedV2Props {
  isDarkMode?: boolean;
}

const SDK_PACKAGES = [
  {
    lang: 'Node',
    cmd: 'npm i iii-sdk',
    href: 'https://www.npmjs.com/package/iii-sdk',
  },
  {
    lang: 'Python',
    cmd: 'pip install iii-sdk',
    href: 'https://pypi.org/project/iii-sdk/',
  },
  {
    lang: 'Rust',
    cmd: 'cargo add iii-sdk',
    href: 'https://crates.io/crates/iii-sdk',
  },
];

const LINKS = [
  { label: 'Quickstart', href: 'https://iii.dev/docs/quickstart' },
  { label: 'Registry', href: 'https://iii.dev/registry' },
  { label: 'GitHub', href: 'https://github.com/iii-hq/iii' },
  { label: 'Spec', href: '/spec' },
];

export function GetStartedV2({ isDarkMode = true }: GetStartedV2Props) {
  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const sectionBg = isDarkMode ? 'bg-iii-dark/30' : 'bg-iii-light';
  const cardBorder = isDarkMode
    ? 'border-iii-light/12'
    : 'border-iii-black/12';
  const cardBg = isDarkMode ? 'bg-iii-dark/50' : 'bg-white/80';
  const linkHover = isDarkMode
    ? 'hover:text-iii-accent'
    : 'hover:text-iii-accent-light';

  return (
    <section className={`w-full font-mono ${sectionBg}`}>
      <div className="mx-auto w-full max-w-6xl px-4 sm:px-6 py-14 md:py-20">
        <div className="max-w-3xl">
          <p className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}>
            Start
          </p>
          <h2
            className={`mt-3 text-3xl sm:text-4xl md:text-5xl font-bold tracking-tighter leading-[1.05] font-chivo ${primary}`}
          >
            Install once.
            <span className={accent}> Add anything next.</span>
          </h2>
        </div>

        <div className="mt-10 grid grid-cols-1 lg:grid-cols-[1.2fr_1fr] gap-5 md:gap-6">
          <div
            className={`rounded-xl border ${cardBorder} ${cardBg} p-6 md:p-7 flex flex-col gap-6`}
          >
            <div>
              <span
                className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}
              >
                01 · Install the engine
              </span>
              <div className="mt-3">
                <InstallShButton isDarkMode={isDarkMode} />
              </div>
            </div>

            <div>
              <span
                className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}
              >
                02 · Add your first worker
              </span>
              <div
                className={`mt-3 px-3 py-2.5 rounded border ${cardBorder} ${
                  isDarkMode ? 'bg-iii-black/40' : 'bg-iii-light'
                }`}
              >
                <code className={`text-xs sm:text-sm ${primary}`}>
                  <span className={muted}>$ </span>
                  iii worker add{' '}
                  <span className={accent}>postgres</span>
                </code>
              </div>
            </div>

            <div>
              <span
                className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}
              >
                03 · Or write your own
              </span>
              <div className="mt-3 grid grid-cols-1 sm:grid-cols-3 gap-2">
                {SDK_PACKAGES.map((pkg) => (
                  <a
                    key={pkg.lang}
                    href={pkg.href}
                    className={`rounded border ${cardBorder} px-3 py-2 hover:border-current transition-colors`}
                  >
                    <div
                      className={`text-[10px] uppercase tracking-[0.28em] ${muted}`}
                    >
                      {pkg.lang}
                    </div>
                    <code className={`text-xs ${primary}`}>{pkg.cmd}</code>
                  </a>
                ))}
              </div>
            </div>
          </div>

          <div
            className={`rounded-xl border ${cardBorder} ${cardBg} p-6 md:p-7 flex flex-col gap-5`}
          >
            <div>
              <span
                className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}
              >
                Stay close
              </span>
              <h3
                className={`mt-2 text-xl md:text-2xl font-bold tracking-tight ${primary}`}
              >
                New workers, every week.
              </h3>
              <p className={`mt-2 text-sm ${secondary}`}>
                Get the registry drops, SDK releases, and protocol updates.
                Low‑volume, dev‑first.
              </p>
            </div>

            <EmailSignupForm isDarkMode={isDarkMode} />

            <div
              className={`flex flex-wrap gap-x-5 gap-y-2 text-xs ${primary}`}
            >
              {LINKS.map((l) => (
                <a
                  key={l.label}
                  href={l.href}
                  className={`inline-flex items-center gap-1 ${linkHover} transition-colors`}
                >
                  {l.label}
                  <span aria-hidden>↗</span>
                </a>
              ))}
            </div>
          </div>
        </div>

        <div
          className={`mt-10 pt-6 border-t ${cardBorder} flex flex-wrap items-center justify-between gap-3 text-[11px] ${muted}`}
        >
          <span className="uppercase tracking-[0.32em]">
            Open protocol · Multiple SDKs · One registry
          </span>
          <span>
            <span className={primary}>spec</span> TBD ·{' '}
            <span className={primary}>engine</span> ELv2 ·{' '}
            <span className={primary}>SDKs</span> Apache‑2.0
          </span>
        </div>
      </div>
    </section>
  );
}
