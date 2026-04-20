import { useState } from 'react';

interface RegistryV2Props {
  isDarkMode?: boolean;
}

type Worker = {
  id: string;
  name: string;
  description: string;
  category:
    | 'storage'
    | 'events'
    | 'ai'
    | 'observability'
    | 'integrations'
    | 'compute';
  version: string;
  installs: string;
  official: boolean;
};

const WORKERS: Worker[] = [
  {
    id: 'iii/postgres',
    name: 'postgres',
    description: 'Postgres queries, transactions, and change streams as functions.',
    category: 'storage',
    version: '1.4.2',
    installs: '24.1k',
    official: true,
  },
  {
    id: 'iii/redis',
    name: 'redis',
    description: 'Key/value, pub-sub, and streams backed by Redis.',
    category: 'storage',
    version: '1.2.0',
    installs: '18.7k',
    official: true,
  },
  {
    id: 'iii/mongo',
    name: 'mongo',
    description: 'MongoDB operations and change streams.',
    category: 'storage',
    version: '0.8.1',
    installs: '6.3k',
    official: true,
  },
  {
    id: 'iii/queue',
    name: 'queue',
    description: 'Durable, ordered queues with retries and DLQ.',
    category: 'events',
    version: '2.0.0',
    installs: '31.4k',
    official: true,
  },
  {
    id: 'iii/stream',
    name: 'stream',
    description: 'Append-only event streams with consumer groups.',
    category: 'events',
    version: '1.6.0',
    installs: '14.2k',
    official: true,
  },
  {
    id: 'iii/cron',
    name: 'cron',
    description: 'Cron scheduler that fires triggers on the right second.',
    category: 'events',
    version: '1.1.0',
    installs: '22.0k',
    official: true,
  },
  {
    id: 'iii/agent',
    name: 'agent',
    description: 'LLM agents as workers — tools, memory, streaming responses.',
    category: 'ai',
    version: '2.1.0',
    installs: '9.8k',
    official: true,
  },
  {
    id: 'iii/browser',
    name: 'browser',
    description: 'Drive a real browser. Functions for navigate, click, scrape.',
    category: 'ai',
    version: '0.9.0',
    installs: '4.5k',
    official: true,
  },
  {
    id: 'iii/sandbox',
    name: 'sandbox',
    description: 'Run untrusted code in isolated, ephemeral sandboxes.',
    category: 'compute',
    version: '0.7.3',
    installs: '3.1k',
    official: true,
  },
  {
    id: 'iii/observability',
    name: 'observability',
    description: 'OTLP traces, logs, and metrics for the whole graph.',
    category: 'observability',
    version: '1.3.0',
    installs: '28.6k',
    official: true,
  },
  {
    id: 'community/stripe',
    name: 'stripe',
    description: 'Charges, subscriptions, and webhook triggers.',
    category: 'integrations',
    version: '1.0.0',
    installs: '5.9k',
    official: false,
  },
  {
    id: 'community/sendgrid',
    name: 'sendgrid',
    description: 'Send transactional email; webhook triggers for events.',
    category: 'integrations',
    version: '0.6.2',
    installs: '2.4k',
    official: false,
  },
  {
    id: 'community/slack',
    name: 'slack',
    description: 'Post messages, react to events, slash commands as triggers.',
    category: 'integrations',
    version: '1.1.0',
    installs: '4.0k',
    official: false,
  },
  {
    id: 'community/openai',
    name: 'openai',
    description: 'Chat, embeddings, and tools — typed function wrappers.',
    category: 'ai',
    version: '1.5.0',
    installs: '11.7k',
    official: false,
  },
];

const CATEGORIES = [
  { key: 'all', label: 'All' },
  { key: 'storage', label: 'Storage' },
  { key: 'events', label: 'Events' },
  { key: 'ai', label: 'AI' },
  { key: 'compute', label: 'Compute' },
  { key: 'observability', label: 'Observability' },
  { key: 'integrations', label: 'Integrations' },
] as const;

type CategoryKey = (typeof CATEGORIES)[number]['key'];

export function RegistryV2({ isDarkMode = true }: RegistryV2Props) {
  const [active, setActive] = useState<CategoryKey>('all');
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const filtered =
    active === 'all' ? WORKERS : WORKERS.filter((w) => w.category === active);

  const copy = (worker: Worker) => {
    navigator.clipboard?.writeText(`iii worker add ${worker.name}`);
    setCopiedId(worker.id);
    setTimeout(() => setCopiedId(null), 1400);
  };

  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const sectionBg = isDarkMode ? 'bg-iii-dark/30' : 'bg-iii-light';
  const cardBorder = isDarkMode
    ? 'border-iii-light/12'
    : 'border-iii-black/12';
  const cardBg = isDarkMode ? 'bg-iii-dark/50' : 'bg-white/80';
  const cardHoverBorder = isDarkMode
    ? 'hover:border-iii-accent/60'
    : 'hover:border-iii-accent-light/60';
  const chipIdle = isDarkMode
    ? 'border-iii-light/15 text-iii-light/60 hover:text-iii-light hover:border-iii-light/30'
    : 'border-iii-black/15 text-iii-black/60 hover:text-iii-black hover:border-iii-black/30';
  const chipActive = isDarkMode
    ? 'border-iii-accent text-iii-accent'
    : 'border-iii-accent-light text-iii-accent-light';
  const codeBg = isDarkMode ? 'bg-iii-black/40' : 'bg-iii-light';

  return (
    <section id="registry" className={`w-full font-mono ${sectionBg}`}>
      <div className="mx-auto w-full max-w-6xl px-4 sm:px-6 py-14 md:py-20">
        <div className="flex flex-col md:flex-row md:items-end md:justify-between gap-6">
          <div className="max-w-2xl">
            <p
              className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}
            >
              Registry
            </p>
            <h2
              className={`mt-3 text-3xl sm:text-4xl md:text-5xl font-bold tracking-tighter leading-[1.05] font-chivo ${primary}`}
            >
              Workers, by{' '}
              <span className={accent}>one command</span>.
            </h2>
            <p className={`mt-4 text-sm md:text-base ${secondary}`}>
              Browse the catalog. Each worker is a runnable capability — a
              binary, a build, or a codebase — that joins your iii engine and
              brings its functions and triggers with it.
            </p>
          </div>

          <a
            href="https://iii.dev/registry"
            className={`text-xs font-bold ${primary} hover:${accent} transition-colors inline-flex items-center gap-1`}
          >
            See all workers <span>→</span>
          </a>
        </div>

        <div className="mt-8 flex flex-wrap gap-2">
          {CATEGORIES.map((cat) => (
            <button
              key={cat.key}
              type="button"
              onClick={() => setActive(cat.key)}
              className={`px-3 py-1.5 rounded-full border text-[11px] tracking-wide transition-colors cursor-pointer ${
                active === cat.key ? chipActive : chipIdle
              }`}
            >
              {cat.label}
            </button>
          ))}
        </div>

        <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 md:gap-4">
          {filtered.map((w) => (
            <article
              key={w.id}
              className={`rounded-xl border ${cardBorder} ${cardBg} ${cardHoverBorder} transition-colors p-5 flex flex-col gap-3`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <h3
                      className={`text-base font-bold tracking-tight truncate ${primary}`}
                    >
                      {w.name}
                    </h3>
                    {w.official && (
                      <span
                        className={`text-[9px] uppercase tracking-[0.22em] px-1.5 py-0.5 rounded ${accent} border ${
                          isDarkMode
                            ? 'border-iii-accent/40'
                            : 'border-iii-accent-light/40'
                        }`}
                      >
                        official
                      </span>
                    )}
                  </div>
                  <p className={`mt-0.5 text-[10px] tracking-wide ${muted}`}>
                    {w.id} · v{w.version}
                  </p>
                </div>
                <span className={`text-[10px] tracking-wide shrink-0 ${muted}`}>
                  {w.installs}/wk
                </span>
              </div>

              <p className={`text-xs leading-relaxed ${secondary}`}>
                {w.description}
              </p>

              <button
                type="button"
                onClick={() => copy(w)}
                className={`mt-auto flex items-center justify-between gap-2 px-3 py-2 rounded ${codeBg} border ${cardBorder} hover:border-current transition-colors text-left cursor-pointer group`}
              >
                <code className={`text-[11px] sm:text-xs ${primary}`}>
                  {copiedId === w.id ? (
                    <span className={`${accent}`}>copied!</span>
                  ) : (
                    <>
                      <span className={muted}>$ </span>
                      iii worker add {w.name}
                    </>
                  )}
                </code>
                <span
                  className={`text-[10px] uppercase tracking-[0.24em] ${muted} group-hover:${primary}`}
                >
                  copy
                </span>
              </button>
            </article>
          ))}
        </div>

        <p className={`mt-6 text-xs ${muted}`}>
          A worker can be a binary, a build, or a codebase. Anything that
          implements the iii protocol can publish to the registry.
        </p>
      </div>
    </section>
  );
}
