import { useState } from 'react';
import { Highlight, themes } from 'prism-react-renderer';

interface CodeV2Props {
  isDarkMode?: boolean;
}

type Lang = 'typescript' | 'python' | 'rust';

const SAMPLES: Record<Lang, { label: string; prismLang: string; code: string }> = {
  typescript: {
    label: 'TypeScript',
    prismLang: 'tsx',
    code: `import { registerWorker } from "iii-sdk";

const iii = registerWorker("ws://localhost:49134");

iii.registerFunction({
  id: "orders::validate",
  handler: async (input) => ({ ok: true }),
});

iii.registerTrigger({
  type: "http",
  function_id: "orders::validate",
  config: { api_path: "/orders/validate", http_method: "POST" },
});
`,
  },
  python: {
    label: 'Python',
    prismLang: 'python',
    code: `from iii import register_worker

iii = register_worker("ws://localhost:49134")

@iii.function(id="orders::validate")
async def validate(input):
    return {"ok": True}

iii.register_trigger({
    "type": "http",
    "function_id": "orders::validate",
    "config": {"api_path": "/orders/validate", "http_method": "POST"},
})
`,
  },
  rust: {
    label: 'Rust',
    prismLang: 'rust',
    code: `use iii_sdk::{register_worker, InitOptions, RegisterFunction,
    IIITrigger, HttpTriggerConfig, HttpMethod};

let iii = register_worker("ws://localhost:49134", InitOptions::default());

iii.register_function(
    RegisterFunction::new("orders::validate", |_input| async move {
        Ok(json!({ "ok": true }))
    }),
);

iii.register_trigger(
    IIITrigger::Http(
        HttpTriggerConfig::new("/orders/validate")
            .method(HttpMethod::Post),
    )
    .for_function("orders::validate"),
);
`,
  },
};

const TABS: Lang[] = ['typescript', 'python', 'rust'];

export function CodeV2({ isDarkMode = true }: CodeV2Props) {
  const [active, setActive] = useState<Lang>('typescript');
  const sample = SAMPLES[active];

  const primary = isDarkMode ? 'text-iii-light' : 'text-iii-black';
  const secondary = isDarkMode ? 'text-iii-light/70' : 'text-iii-black/70';
  const muted = isDarkMode ? 'text-iii-light/40' : 'text-iii-black/40';
  const accent = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const panelBorder = isDarkMode
    ? 'border-iii-light/12'
    : 'border-iii-black/12';
  const panelBg = isDarkMode ? 'bg-iii-dark/50' : 'bg-white/80';
  const tabIdle = isDarkMode
    ? 'text-iii-light/50 hover:text-iii-light'
    : 'text-iii-black/50 hover:text-iii-black';
  const tabActive = isDarkMode ? 'text-iii-accent' : 'text-iii-accent-light';
  const tabUnderlineActive = isDarkMode
    ? 'bg-iii-accent'
    : 'bg-iii-accent-light';
  const sectionBg = isDarkMode ? 'bg-iii-dark/30' : 'bg-iii-light';

  const prismTheme = isDarkMode ? themes.vsDark : themes.vsLight;

  return (
    <section className={`w-full font-mono ${sectionBg}`}>
      <div className="mx-auto w-full max-w-6xl px-4 sm:px-6 py-14 md:py-20">
        <div className="grid grid-cols-1 lg:grid-cols-[1.1fr_1.4fr] gap-8 md:gap-12 items-start">
          <div>
            <p className={`text-[10px] uppercase tracking-[0.32em] ${muted}`}>
              Build your own
            </p>
            <h2
              className={`mt-3 text-3xl sm:text-4xl md:text-5xl font-bold tracking-tighter leading-[1.05] font-chivo ${primary}`}
            >
              Your code is a{' '}
              <span className={accent}>worker</span> too.
            </h2>
            <p className={`mt-4 text-sm md:text-base ${secondary}`}>
              Register a function. Bind a trigger. Connect. The moment you do,
              your service joins the same registry as everything else — and
              becomes callable from any other worker, in any language.
            </p>
            <ul className={`mt-6 space-y-2.5 text-sm ${secondary}`}>
              <li>
                <span className={accent}>·</span> No client libraries to
                publish — your function ID is the contract.
              </li>
              <li>
                <span className={accent}>·</span> No deploy ceremony — workers
                connect, register, and disappear without config.
              </li>
              <li>
                <span className={accent}>·</span> No glue code — HTTP, cron,
                queues, and streams are first‑class triggers.
              </li>
            </ul>
          </div>

          <div
            className={`rounded-xl border ${panelBorder} ${panelBg} overflow-hidden`}
          >
            <div className="flex items-center justify-between px-4 sm:px-5 pt-3">
              <div className="flex gap-1">
                {TABS.map((lang) => {
                  const isActive = active === lang;
                  return (
                    <button
                      key={lang}
                      type="button"
                      onClick={() => setActive(lang)}
                      className={`relative px-3 py-2 text-xs sm:text-sm font-bold tracking-tight transition-colors cursor-pointer ${
                        isActive ? tabActive : tabIdle
                      }`}
                    >
                      {SAMPLES[lang].label}
                      <span
                        className={`absolute left-2 right-2 -bottom-px h-px transition-opacity ${
                          isActive ? tabUnderlineActive : 'opacity-0'
                        }`}
                      />
                    </button>
                  );
                })}
              </div>
              <span
                className={`text-[10px] uppercase tracking-[0.28em] ${muted}`}
              >
                register · function · trigger
              </span>
            </div>

            <div
              className={`border-t ${panelBorder} px-4 sm:px-6 py-5 overflow-x-auto`}
            >
              <Highlight
                theme={prismTheme}
                code={sample.code.trim()}
                language={sample.prismLang as 'tsx'}
              >
                {({
                  className,
                  style,
                  tokens,
                  getLineProps,
                  getTokenProps,
                }) => (
                  <pre
                    className={`${className} text-[12px] sm:text-[13px] md:text-sm leading-relaxed`}
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
                            return <span key={tokenKey ?? k} {...tokenProps} />;
                          })}
                        </div>
                      );
                    })}
                  </pre>
                )}
              </Highlight>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
