import { useEffect, useState } from 'react';
import { InstallShButton } from '../InstallShButton';

interface HeroV2Props {
  isDarkMode?: boolean;
}

type InstalledLine = {
  worker: string;
  version: string;
};

const INSTALLED: InstalledLine[] = [
  { worker: 'postgres', version: '1.4.2' },
  { worker: 'browser', version: '0.9.0' },
  { worker: 'agent', version: '2.1.0' },
];

const NEXT_WORKERS = [
  'stripe',
  'sendgrid',
  'slack',
  'sandbox',
  'openai',
  'kafka',
  'temporal',
  'anything',
];

export function HeroV2({ isDarkMode = true }: HeroV2Props) {
  const [nextIdx, setNextIdx] = useState(0);
  const [typed, setTyped] = useState('');
  const [phase, setPhase] = useState<'typing' | 'pause' | 'erasing'>('typing');

  useEffect(() => {
    const target = NEXT_WORKERS[nextIdx];
    let timeout: ReturnType<typeof setTimeout>;

    if (phase === 'typing') {
      if (typed.length < target.length) {
        timeout = setTimeout(
          () => setTyped(target.slice(0, typed.length + 1)),
          60,
        );
      } else {
        timeout = setTimeout(() => setPhase('pause'), 1400);
      }
    } else if (phase === 'pause') {
      timeout = setTimeout(() => setPhase('erasing'), 0);
    } else {
      if (typed.length > 0) {
        timeout = setTimeout(
          () => setTyped(typed.slice(0, typed.length - 1)),
          30,
        );
      } else {
        setNextIdx((i) => (i + 1) % NEXT_WORKERS.length);
        setPhase('typing');
      }
    }

    return () => clearTimeout(timeout);
  }, [typed, phase, nextIdx]);

  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const success = 'text-iii-success';
  const promptColor = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const termBg = isDarkMode ? 'bg-iii-dark/80' : 'bg-white';
  const termBorder = isDarkMode
    ? 'border-iii-light/15'
    : 'border-iii-black/15';
  const termHeader = isDarkMode ? 'bg-iii-black/60' : 'bg-iii-light';

  return (
    <section className={`relative w-full font-mono`}>
      <div className="relative z-10 w-full max-w-6xl mx-auto px-4 sm:px-6 py-10 md:py-16">
        <div className="flex flex-col items-center text-center gap-7 md:gap-9">
          <div
            className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-full border text-[10px] sm:text-xs uppercase tracking-[0.28em] ${termBorder} ${muted}`}
          >
            <span
              className={`inline-block w-1.5 h-1.5 rounded-full ${
                isDarkMode ? 'bg-iii-accent' : 'bg-iii-accent-light'
              }`}
            />
            <span>npm install for backend workers</span>
          </div>

          <div
            className={`w-full max-w-3xl rounded-xl border ${termBorder} ${termBg} overflow-hidden text-left shadow-2xl shadow-black/20`}
          >
            <div
              className={`flex items-center gap-2 px-4 py-2.5 border-b ${termBorder} ${termHeader}`}
            >
              <div className="flex gap-1.5">
                <span className="w-2.5 h-2.5 rounded-full bg-red-500/80" />
                <span className="w-2.5 h-2.5 rounded-full bg-yellow-500/80" />
                <span className="w-2.5 h-2.5 rounded-full bg-green-500/80" />
              </div>
              <span
                className={`ml-3 text-[10px] uppercase tracking-[0.28em] ${muted}`}
              >
                ~ /your-app
              </span>
            </div>

            <div className="px-4 sm:px-5 py-4 sm:py-5 text-[12px] sm:text-sm leading-relaxed">
              {INSTALLED.map(({ worker, version }, i) => (
                <div
                  key={worker}
                  className="grid grid-cols-[auto_1fr] gap-x-2"
                  style={{ marginTop: i === 0 ? 0 : '0.5rem' }}
                >
                  <div className="contents">
                    <span className={promptColor}>$</span>
                    <span className={primary}>
                      iii worker add{' '}
                      <span className={accent}>{worker}</span>
                    </span>
                  </div>
                  <div className="contents">
                    <span className={success}>✓</span>
                    <span className={secondary}>
                      added <span className={primary}>{worker}</span>@
                      <span className={muted}>{version}</span>
                    </span>
                  </div>
                </div>
              ))}

              <div
                className="grid grid-cols-[auto_1fr] gap-x-2"
                style={{ marginTop: '0.85rem' }}
              >
                <span className={promptColor}>$</span>
                <span className={primary}>
                  iii worker add{' '}
                  <span className={accent}>{typed}</span>
                  <span
                    className={`inline-block w-[0.5em] -mb-[0.1em] ml-[1px] ${
                      isDarkMode ? 'bg-iii-accent' : 'bg-iii-accent-light'
                    } animate-pulse`}
                    style={{ height: '1.05em', verticalAlign: 'middle' }}
                  />
                </span>
              </div>
            </div>
          </div>

          <h1
            className={`font-chivo font-bold tracking-tighter leading-[0.95] text-[clamp(1.875rem,5.5vw,4.25rem)] ${primary}`}
          >
            Add anything to your backend.
            <br />
            <span className={accent}>One command.</span>
          </h1>

          <p
            className={`max-w-[64ch] text-base sm:text-lg leading-relaxed ${secondary}`}
          >
            iii is a registry of workers — databases, agents, browsers,
            sandboxes, third‑party APIs, anything that does work. Install one,
            install ten, and call them as one system.
          </p>

          <div className="flex flex-col sm:flex-row gap-3 sm:gap-4 items-center justify-center w-full max-w-xl">
            <InstallShButton isDarkMode={isDarkMode} />
            <a
              href="#registry"
              className={`group inline-flex items-center justify-center gap-2 px-4 py-2.5 md:py-3 rounded border text-xs md:text-sm font-bold transition-colors w-full sm:w-auto ${
                isDarkMode
                  ? 'border-iii-light/30 text-iii-light hover:border-iii-accent hover:text-iii-accent'
                  : 'border-iii-black/30 text-iii-black hover:border-iii-accent-light hover:text-iii-accent-light'
              }`}
            >
              Browse the registry
              <span className="transition-transform group-hover:translate-x-0.5">
                →
              </span>
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}
