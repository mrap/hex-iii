import { Highlight, themes } from 'prism-react-renderer';

interface TourV2Props {
  isDarkMode?: boolean;
}

const FUNCTION_CODE = `// orders.ts
import { registerWorker } from "iii-sdk";

const iii = registerWorker("ws://localhost:49134");

iii.registerFunction({
  id: "orders::handle",
  handler: async ({ id }) => {
    const order      = await iii.trigger("postgres::query", { sql: "...", id });
    const screenshot = await iii.trigger("browser::screenshot", { url: order.url });
    const reply      = await iii.trigger("agent::reply", { order, screenshot });
    return reply;
  },
});
`;

const ADDED = [
  { worker: 'postgres', version: '1.4.2' },
  { worker: 'browser', version: '0.9.0' },
  { worker: 'agent', version: '2.1.0' },
];

const TRACE = [
  { name: 'postgres::query', total: '142ms', bar: 12 },
  { name: 'browser::screenshot', total: '876ms', bar: 72 },
  { name: 'agent::reply', total: '182ms', bar: 15 },
];

export function TourV2({ isDarkMode = true }: TourV2Props) {
  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const accentBg = isDarkMode ? 'bg-iii-accent' : 'bg-iii-accent-light';
  const success = 'text-iii-success';
  const sectionBg = isDarkMode ? 'bg-iii-black' : 'bg-iii-light';
  const cardBorder = isDarkMode
    ? 'border-iii-light/12'
    : 'border-iii-black/12';
  const cardBg = isDarkMode ? 'bg-iii-dark/50' : 'bg-white/80';
  const codeBg = isDarkMode ? 'bg-iii-dark/70' : 'bg-white';
  const stepBorder = isDarkMode
    ? 'border-iii-light/10'
    : 'border-iii-black/10';
  const promptColor = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const trackBg = isDarkMode ? 'bg-iii-light/10' : 'bg-iii-black/10';

  const prismTheme = isDarkMode ? themes.vsDark : themes.vsLight;

  return (
    <section className={`w-full font-mono ${sectionBg}`}>
      <div className="mx-auto w-full max-w-6xl px-4 sm:px-6 py-14 md:py-20">
        <div className="max-w-3xl">
          <p className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}>
            60 seconds
          </p>
          <h2
            className={`mt-3 text-3xl sm:text-4xl md:text-5xl font-bold tracking-tighter leading-[1.05] font-chivo ${primary}`}
          >
            Three workers,
            <br />
            <span className={accent}>one function</span>,{' '}
            <span className={muted}>·</span> one trace.
          </h2>
          <p className={`mt-4 text-sm md:text-base ${secondary}`}>
            Add a database, a browser, and an agent. Compose them in a single
            function. Watch one trace flow across three different worlds.
          </p>
        </div>

        <div
          className={`mt-10 md:mt-14 rounded-xl border ${cardBorder} ${cardBg} overflow-hidden divide-y ${stepBorder}`}
        >
          <Step
            num="01"
            title="Add three workers"
            note="Each is a runnable capability. One command, no config."
            muted={muted}
            primary={primary}
            accent={accent}
            stepBorder={stepBorder}
          >
            <div
              className={`${codeBg} px-4 sm:px-5 py-4 text-[12px] sm:text-sm leading-[1.7]`}
            >
              {ADDED.map((line, i) => (
                <div
                  key={line.worker}
                  style={{ marginTop: i === 0 ? 0 : '0.4rem' }}
                >
                  <div>
                    <span className={promptColor}>$ </span>
                    <span className={primary}>
                      iii worker add{' '}
                      <span className={accent}>{line.worker}</span>
                    </span>
                  </div>
                  <div>
                    <span className={success}>✓ </span>
                    <span className={secondary}>
                      added <span className={primary}>{line.worker}</span>@
                      <span className={muted}>{line.version}</span>
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </Step>

          <Step
            num="02"
            title="Call them like one system"
            note="Same protocol. Same call shape. Different worlds behind it."
            muted={muted}
            primary={primary}
            accent={accent}
            stepBorder={stepBorder}
          >
            <div className={`${codeBg} px-4 sm:px-5 py-4 overflow-x-auto`}>
              <Highlight
                theme={prismTheme}
                code={FUNCTION_CODE.trim()}
                language="tsx"
              >
                {({ className, style, tokens, getLineProps, getTokenProps }) => (
                  <pre
                    className={`${className} text-[12px] sm:text-[13px] leading-relaxed`}
                    style={{
                      ...style,
                      background: 'transparent',
                      margin: 0,
                      padding: 0,
                    }}
                  >
                    {tokens.map((line, i) => {
                      const { key: lineKey, ...lineProps } = getLineProps({
                        line,
                        key: i,
                      });
                      return (
                        <div key={lineKey ?? i} {...lineProps}>
                          <span
                            className={`inline-block w-7 select-none text-right pr-3 ${muted}`}
                          >
                            {i + 1}
                          </span>
                          {line.map((token, k) => {
                            const { key: tokenKey, ...tokenProps } =
                              getTokenProps({ token, key: k });
                            return (
                              <span key={tokenKey ?? k} {...tokenProps} />
                            );
                          })}
                        </div>
                      );
                    })}
                  </pre>
                )}
              </Highlight>
            </div>
          </Step>

          <Step
            num="03"
            title="Invoke. See one trace."
            note="Postgres, browser, and LLM observed as one graph."
            muted={muted}
            primary={primary}
            accent={accent}
            stepBorder={stepBorder}
          >
            <div
              className={`${codeBg} px-4 sm:px-5 py-4 text-[12px] sm:text-sm leading-[1.7]`}
            >
              <div>
                <span className={promptColor}>$ </span>
                <span className={primary}>
                  iii fn invoke{' '}
                  <span className={accent}>orders::handle</span>{' '}
                  --input{' '}
                  <span className={secondary}>{`'{"id":"ord_1"}'`}</span>
                </span>
              </div>
              <div className="mt-2">
                <span className={success}>✓</span>{' '}
                <span className={secondary}>
                  orders::handle{' '}
                  <span className={muted}>returned in</span>{' '}
                  <span className={primary}>1.21s</span>
                </span>
              </div>
              <div className="mt-3 space-y-1.5">
                {TRACE.map((t, idx) => (
                  <div
                    key={t.name}
                    className="grid grid-cols-[1.25rem_minmax(8.5rem,12rem)_1fr_auto] items-center gap-x-3"
                  >
                    <span className={muted}>
                      {idx === TRACE.length - 1 ? '└─' : '├─'}
                    </span>
                    <span className={`${primary} truncate`}>{t.name}</span>
                    <div className={`relative h-1.5 rounded-full ${trackBg}`}>
                      <div
                        className={`absolute inset-y-0 left-0 rounded-full ${accentBg}`}
                        style={{ width: `${t.bar}%` }}
                      />
                    </div>
                    <span className={`${muted} tabular-nums shrink-0`}>
                      {t.total}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </Step>
        </div>

        <p className={`mt-8 text-sm ${secondary}`}>
          Add another worker tomorrow — Stripe, a sandbox, an MCP server. The
          shape of the call doesn't change. Neither does the trace.
        </p>
      </div>
    </section>
  );
}

function Step({
  num,
  title,
  note,
  muted,
  primary,
  accent,
  stepBorder,
  children,
}: {
  num: string;
  title: string;
  note: string;
  muted: string;
  primary: string;
  accent: string;
  stepBorder: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <header className={`px-5 sm:px-6 py-4 sm:py-5 border-b ${stepBorder}`}>
        <div className="flex items-baseline gap-3">
          <span className={`text-[11px] tracking-[0.28em] ${accent}`}>
            {num}
          </span>
          <div className="min-w-0">
            <h3 className={`text-base sm:text-lg font-bold tracking-tight ${primary}`}>
              {title}
            </h3>
            <p className={`text-[11px] sm:text-xs ${muted}`}>{note}</p>
          </div>
        </div>
      </header>
      {children}
    </div>
  );
}
