interface ModelV2Props {
  isDarkMode?: boolean;
}

type Primitive = {
  number: string;
  name: string;
  oneLiner: string;
  examples: string[];
};

const PRIMITIVES: Primitive[] = [
  {
    number: '01',
    name: 'Function',
    oneLiner: 'Anything callable.',
    examples: ['orders::validate', 'postgres::query', 'agent::reply'],
  },
  {
    number: '02',
    name: 'Trigger',
    oneLiner: 'Anything that runs a Function.',
    examples: ['HTTP', 'cron', 'queue', 'state', 'stream'],
  },
  {
    number: '03',
    name: 'Worker',
    oneLiner: 'Anything that hosts Functions or emits Triggers.',
    examples: ['service', 'agent', 'browser', 'device', 'API'],
  },
];

export function ModelV2({ isDarkMode = true }: ModelV2Props) {
  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const sectionBg = isDarkMode ? 'bg-iii-black' : 'bg-iii-light';
  const cardBorder = isDarkMode
    ? 'border-iii-light/12'
    : 'border-iii-black/12';
  const chipBorder = isDarkMode
    ? 'border-iii-light/15'
    : 'border-iii-black/15';

  return (
    <section className={`w-full font-mono ${sectionBg}`}>
      <div className="mx-auto w-full max-w-6xl px-4 sm:px-6 py-12 md:py-16">
        <div className="flex flex-col md:flex-row md:items-end md:justify-between gap-4 md:gap-8">
          <div className="max-w-2xl">
            <p className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}>
              Why it composes
            </p>
            <h2
              className={`mt-2 text-2xl sm:text-3xl md:text-4xl font-bold tracking-tighter leading-[1.1] font-chivo ${primary}`}
            >
              Three primitives.{' '}
              <span className={accent}>Nothing else.</span>
            </h2>
          </div>
          <p className={`text-sm md:text-base max-w-md ${secondary}`}>
            Every worker — official, community, your own — speaks the same
            three nouns. That's why everything you add composes with everything
            you already had.
          </p>
        </div>

        <div className={`mt-8 grid grid-cols-1 md:grid-cols-3 gap-px rounded-xl border ${cardBorder} overflow-hidden`}>
          {PRIMITIVES.map((p, idx) => (
            <article
              key={p.name}
              className={`p-5 md:p-6 ${
                isDarkMode ? 'bg-iii-dark/40' : 'bg-white/70'
              } ${idx > 0 ? `md:border-l ${cardBorder} border-t md:border-t-0 ${cardBorder}` : ''}`}
            >
              <div className="flex items-baseline gap-3">
                <span className={`text-[11px] tracking-[0.28em] ${accent}`}>
                  {p.number}
                </span>
                <h3 className={`text-lg font-bold tracking-tight ${primary}`}>
                  {p.name}
                </h3>
              </div>
              <p className={`mt-3 text-sm ${primary}`}>{p.oneLiner}</p>
              <div className="mt-4 flex flex-wrap gap-1.5">
                {p.examples.map((ex) => (
                  <span
                    key={ex}
                    className={`text-[10px] px-2 py-0.5 rounded border ${chipBorder} ${muted}`}
                  >
                    {ex}
                  </span>
                ))}
              </div>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}
