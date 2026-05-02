# Nightly AI Verdict Review Workflow — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a nightly GitHub Actions workflow that runs `anthropics/claude-code-action@v1` against the iii repo with a fresh context, parses the resulting architectural verdict, and posts a finding count + day-over-day delta to the existing release Slack channel.

**Architecture:** Single-job GitHub Actions workflow on a `cron: '0 7 * * *'` schedule. The workflow loads a static prompt from `.github/verdict-prompt.md` into env, runs the Claude Code Action with Opus 4.7, then a Python extractor (stdlib only) parses the action's `execution_file` JSON to produce `verdict.md` (the human-readable critique) and `verdict.json` (the machine-readable findings list). Both are uploaded as a 90-day artifact. A separate workflow step downloads the previous run's `verdict.json` (best-effort via `gh run download`) to compute a `(-N vs yesterday)` delta string for the Slack header — the model itself never sees prior verdict text. `if: success()` posts the verdict; `if: failure()` posts a "run FAILED" ping to the same channel so silence cannot hide drift. Notify-only — workflow always exits 0; no CI gate.

**Tech Stack:** GitHub Actions, `anthropics/claude-code-action@v1`, `slackapi/slack-github-action@v2.0.0`, Python 3 stdlib (extractor), pytest ≥ 8 (extractor tests), `actionlint` (YAML validation), `gh` CLI (artifact download).

---

## File Structure

| Path | Responsibility | Lines |
|------|----------------|-------|
| `.github/workflows/verdict-review.yml` | Workflow: schedule, prompt load, action invocation, prev-count fetch, extraction, artifact upload, Slack success ping, Slack failure ping | ~115 |
| `.github/verdict-prompt.md` | The senior-engineer review brief; defines the strict JSON contract for the extractor | ~50 |
| `.github/scripts/extract_verdict.py` | Parse `execution_file` → write `verdict.md` + `verdict.json`; defensive fallbacks for missing/malformed input; 50KB body cap | ~75 |
| `.github/scripts/test_extract_verdict.py` | pytest suite — 9 tests covering 7 scenarios: happy path, malformed JSON tail, missing JSON tail, missing/empty/none exec file, mid-body JSON noise, body truncation, zero-findings goal state | ~140 |
| `.github/workflows/WORKFLOWS.md` | Add workflow row, dedicated section, secret entry | edit only |

Each file has one responsibility. The extractor's JSON contract (defined in the prompt, parsed by the Python script, formatted into Slack by the workflow) is the only cross-file coupling — pin it down in Task 1 and don't drift.

---

## Pre-flight

**Branch:** create a feature branch from `main` before starting.

```bash
cd /Users/ytallolayon/workspaces/personal/motia/iii
git checkout main && git pull
git checkout -b feat/verdict-review-workflow
```

**Local tooling check:**

```bash
python3 --version       # ≥ 3.10
python3 -m pytest --version 2>/dev/null || pip install pytest
which actionlint || brew install actionlint
which jq || brew install jq
which gh && gh auth status
```

If any of those are missing, install before starting Task 1. The extractor uses stdlib only; pytest is needed only for running the test suite.

---

## Task 1: Verdict prompt + scripts directory scaffolding

**Files:**
- Create: `.github/scripts/` (directory)
- Create: `.github/verdict-prompt.md`
- Create: `.github/scripts/__init__.py` (empty marker so pytest discovers the test file as a module)

- [ ] **Step 1: Create the scripts directory**

```bash
mkdir -p .github/scripts
touch .github/scripts/__init__.py
```

- [ ] **Step 2: Write `.github/verdict-prompt.md`**

```markdown
You are a senior staff engineer doing an independent technical review of the
iii repository (Rust engine + Node/Python/Rust SDKs + console). Treat this
checkout as the only source of truth — you have no memory of any prior reviews.

## What to investigate

Walk the repo with Read/Glob/Grep/Bash(rg|fd|cat|jq) tools and form an opinion on:

- Architectural coherence (engine ↔ SDK protocol match, registry/HA story,
  worker lifecycle, state handling)
- Concrete mismatches between SDK surface and engine wire protocol
  (look for message types declared in SDKs but absent from `engine/src/protocol.rs`)
- License consistency (engine ELv2 vs SDKs/docs Apache-2.0)
- CI maturity signals (commented-out lints, skipped jobs, missing CHANGELOG,
  broken doc links)
- Test coverage gaps in the engine's hot paths
- Concentration risk (oversized files, e.g. `engine/src/engine/mod.rs`)
- Stale/missing documentation referenced from README/docs

## What to ignore

- Cosmetic style nits a linter would catch
- Anything you can't verify from the source tree itself (market positioning, funding, hiring plans, customer adoption, vendor relationships)
- Findings without a file:line citation

## Output format — STRICT

Produce a Markdown verdict with these sections, in this order:

1. **Verdict** (one paragraph)
2. **What Looks Strong** (bullets with `path:line` citations)
3. **What Would Concern Me** (bullets, each tagged `[critical|high|medium|low]`,
   each with a `path:line` citation)
4. **Where I'd Use It / Avoid It** (one paragraph)

Then, on the very last line of your response, emit a single line of JSON
(no code fence, no prose, no trailing newline) matching this schema:

`{"score": 0-100, "findings": [{"severity": "critical|high|medium|low", "title": "...", "citation": "path:line"}]}`

`findings` is the deduplicated list from section 3. `score` is your confidence
that iii is production-ready (100 = ship it, 0 = don't). Goal: 0 findings.
```

- [ ] **Step 3: Verify file shape**

Run:

```bash
wc -l .github/verdict-prompt.md
head -3 .github/verdict-prompt.md
```

Expected: ~40-50 lines; first line begins with "You are a senior staff engineer".

- [ ] **Step 4: Commit**

```bash
git add .github/verdict-prompt.md .github/scripts/__init__.py
git commit -m "feat(verdict-review): add prompt + scripts dir scaffolding"
```

---

## Task 2: Extractor — happy path (TDD)

**Files:**
- Create: `.github/scripts/extract_verdict.py`
- Create: `.github/scripts/test_extract_verdict.py`

- [ ] **Step 1: Write the failing test (case 1: happy path)**

Create `.github/scripts/test_extract_verdict.py`:

```python
"""Tests for extract_verdict.py — 9 tests covering the 7 scenarios in the plan."""
import json
import sys
from pathlib import Path

# Make the script importable as a module
sys.path.insert(0, str(Path(__file__).parent))
import extract_verdict  # noqa: E402


def _write_exec(tmp_path, assistant_text):
    """Write a synthetic execution_file containing a single assistant message."""
    fixture = tmp_path / "exec.json"
    fixture.write_text(json.dumps([{"role": "assistant", "content": assistant_text}]))
    return fixture


def _run(tmp_path, exec_path):
    md = tmp_path / "verdict.md"
    j = tmp_path / "verdict.json"
    rc = extract_verdict.main(str(exec_path) if exec_path else None, str(md), str(j))
    return rc, md, j


def test_happy_path(tmp_path):
    """Case 1: well-formed transcript, valid JSON tail, multiple findings."""
    body = (
        "## Verdict\n\nLooks great.\n\n"
        "## Findings\n\n- [high] one issue (a.rs:1)\n"
    )
    contract = '{"score":85,"findings":[{"severity":"high","title":"one issue","citation":"a.rs:1"}]}'
    exec_path = _write_exec(tmp_path, body + "\n" + contract)

    rc, md, j = _run(tmp_path, exec_path)

    assert rc == 0
    assert "Looks great" in md.read_text()
    assert "one issue" in md.read_text()
    data = json.loads(j.read_text())
    assert data["score"] == 85
    assert len(data["findings"]) == 1
    assert data["findings"][0]["severity"] == "high"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py::test_happy_path -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'extract_verdict'`.

- [ ] **Step 3: Write minimal implementation**

Create `.github/scripts/extract_verdict.py`:

```python
"""Parse claude-code-action execution_file → verdict.md + verdict.json.

Stdlib only. Defensive against missing files, malformed JSON, oversized bodies,
and unexpected transcript shapes. Always exits 0 — the workflow's if:failure()
step handles user-visible failure signaling.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Iterator

MAX_BODY_BYTES = 50 * 1024  # Slack-friendly cap

FALLBACK_NO_TRANSCRIPT = {
    "score": 0,
    "findings": [
        {
            "severity": "high",
            "title": "verdict workflow failed before producing transcript",
            "citation": "n/a",
        }
    ],
}

FALLBACK_NO_CONTRACT = {
    "score": 0,
    "findings": [
        {
            "severity": "high",
            "title": "verdict transcript missing JSON contract",
            "citation": "n/a",
        }
    ],
}


def _walk_strings(obj: Any) -> Iterator[str]:
    """Recursively yield every string leaf in obj."""
    if isinstance(obj, str):
        yield obj
    elif isinstance(obj, list):
        for item in obj:
            yield from _walk_strings(item)
    elif isinstance(obj, dict):
        for value in obj.values():
            yield from _walk_strings(value)


def _split_contract(text: str) -> tuple[str, dict | None]:
    """Split body from trailing JSON contract line.

    Returns (body, parsed) on success; (text, None) if the last non-empty line
    is not valid JSON matching the {score, findings} schema.
    """
    lines = text.rstrip().splitlines()
    if not lines:
        return text, None
    last = lines[-1].strip()
    try:
        parsed = json.loads(last)
    except (json.JSONDecodeError, ValueError):
        return text, None
    if not (isinstance(parsed, dict) and "score" in parsed and "findings" in parsed):
        return text, None
    body = "\n".join(lines[:-1]).rstrip()
    return body, parsed


def _load_transcript(path: Path) -> Any:
    """Load execution_file as JSON; fall back to JSONL on parse failure."""
    raw = path.read_text()
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        out = []
        for line in raw.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
        return out


def main(exec_file: str | None, out_md: str, out_json: str) -> int:
    p = Path(exec_file) if exec_file else None
    if p is None or not p.exists() or p.stat().st_size == 0:
        Path(out_md).write_text(
            "# Verdict failed\n\nWorkflow did not produce a transcript.\n"
        )
        Path(out_json).write_text(json.dumps(FALLBACK_NO_TRANSCRIPT))
        return 0

    data = _load_transcript(p)

    # Pick the longest string whose tail parses as the JSON contract.
    best_body: str | None = None
    best_findings: dict | None = None
    for s in _walk_strings(data):
        body, findings = _split_contract(s)
        if findings is None:
            continue
        if best_body is None or len(body) > len(best_body):
            best_body = body
            best_findings = findings

    if best_findings is None:
        # Preserve any substantial text we found, but emit fallback findings.
        candidates = [s for s in _walk_strings(data) if len(s) > 50]
        body = "\n\n---\n\n".join(candidates) if candidates else (
            "Verdict transcript present but no valid JSON contract found."
        )
        findings = FALLBACK_NO_CONTRACT
    else:
        body = best_body
        findings = best_findings

    if len(body.encode("utf-8")) > MAX_BODY_BYTES:
        # Truncate by bytes to be Slack-safe.
        body = body.encode("utf-8")[:MAX_BODY_BYTES].decode("utf-8", errors="ignore")
        body += "\n\n...[truncated to 50KB; see full artifact]"

    Path(out_md).write_text(body + "\n")
    Path(out_json).write_text(json.dumps(findings))
    return 0


if __name__ == "__main__":
    sys.exit(
        main(
            sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else None,
            sys.argv[2],
            sys.argv[3],
        )
    )
```

- [ ] **Step 4: Run test to verify it passes**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py::test_happy_path -v
```

Expected: PASS (1 passed).

- [ ] **Step 5: Commit**

```bash
git add .github/scripts/extract_verdict.py .github/scripts/test_extract_verdict.py
git commit -m "feat(verdict-review): extractor happy path + first test"
```

---

## Task 3: Extractor — malformed and missing JSON tail (TDD, 3 cases)

**Files:**
- Modify: `.github/scripts/test_extract_verdict.py` (append 3 tests)

The extractor already handles these via `_split_contract` returning `None` and the fallback branch — these tests verify that behavior. Expect all three to pass on first run; if any fails the implementation has a bug.

- [ ] **Step 1: Add three failing tests (case 2, 3, 5)**

Append to `.github/scripts/test_extract_verdict.py`:

```python


def test_malformed_json_tail(tmp_path):
    """Case 2: body present but trailing JSON is broken — fallback fires."""
    body = "## Verdict\n\nGood stuff.\n"
    bad_tail = "{score: BROKEN"
    exec_path = _write_exec(tmp_path, body + "\n" + bad_tail)

    _, md, j = _run(tmp_path, exec_path)

    data = json.loads(j.read_text())
    assert any("missing JSON contract" in f["title"] for f in data["findings"])
    # Body is preserved (we kept the substantial text)
    assert "Good stuff" in md.read_text()


def test_missing_json_tail(tmp_path):
    """Case 3: no machine contract emitted at all."""
    body = "## Verdict\n\nNo machine contract emitted.\n"
    exec_path = _write_exec(tmp_path, body)

    _, md, j = _run(tmp_path, exec_path)

    data = json.loads(j.read_text())
    assert "missing JSON contract" in data["findings"][0]["title"]
    assert "No machine contract" in md.read_text()


def test_json_lines_mid_body_malformed_tail(tmp_path):
    """Case 5: JSON-looking lines exist mid-body, but the LAST line is not the contract."""
    text = (
        "## Findings\n\n"
        "```json\n"
        '{"example":"in code block — not the contract"}\n'
        "```\n\n"
        "The end.\n"
        "{not-json"
    )
    exec_path = _write_exec(tmp_path, text)

    _, md, j = _run(tmp_path, exec_path)

    data = json.loads(j.read_text())
    # Only the literal last line counts. Body present but no contract recognized.
    assert "missing JSON contract" in data["findings"][0]["title"]
```

- [ ] **Step 2: Run tests**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py -v
```

Expected: 4 passed (the original happy path + 3 new cases).

- [ ] **Step 3: Commit**

```bash
git add .github/scripts/test_extract_verdict.py
git commit -m "test(verdict-review): cover malformed/missing/mid-body JSON contract"
```

---

## Task 4: Extractor — missing execution_file (TDD)

**Files:**
- Modify: `.github/scripts/test_extract_verdict.py` (append 1 test)

- [ ] **Step 1: Add the failing test (case 4)**

Append to `.github/scripts/test_extract_verdict.py`:

```python


def test_missing_exec_file(tmp_path):
    """Case 4: exec file path doesn't exist (action failed pre-write).

    Must exit 0 (the workflow's if:failure() handles user-visible signaling)
    and write a fallback verdict.
    """
    nonexistent = tmp_path / "not-there.json"
    rc, md, j = _run(tmp_path, nonexistent)

    assert rc == 0
    data = json.loads(j.read_text())
    assert "failed before producing transcript" in data["findings"][0]["title"]
    assert "Workflow did not produce a transcript" in md.read_text()


def test_empty_exec_file(tmp_path):
    """Case 4b: exec file exists but is empty."""
    empty = tmp_path / "empty.json"
    empty.write_text("")
    rc, md, j = _run(tmp_path, empty)

    assert rc == 0
    data = json.loads(j.read_text())
    assert "failed before producing transcript" in data["findings"][0]["title"]


def test_none_exec_file(tmp_path):
    """Case 4c: exec_file argument is None (e.g. action output never set)."""
    rc, md, j = _run(tmp_path, None)

    assert rc == 0
    data = json.loads(j.read_text())
    assert "failed before producing transcript" in data["findings"][0]["title"]
```

- [ ] **Step 2: Run tests**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py -v
```

Expected: 7 passed.

- [ ] **Step 3: Commit**

```bash
git add .github/scripts/test_extract_verdict.py
git commit -m "test(verdict-review): cover missing/empty/none exec_file"
```

---

## Task 5: Extractor — body size cap (TDD)

**Files:**
- Modify: `.github/scripts/test_extract_verdict.py` (append 1 test)

- [ ] **Step 1: Add the failing test (case 6)**

Append to `.github/scripts/test_extract_verdict.py`:

```python


def test_body_truncation(tmp_path):
    """Case 6: body > 50KB is truncated with a marker; full file remains in artifact upload."""
    big_body = "X" * 100_000  # 100KB — well over the cap
    contract = '{"score":50,"findings":[]}'
    exec_path = _write_exec(tmp_path, big_body + "\n" + contract)

    _, md, j = _run(tmp_path, exec_path)

    body = md.read_text()
    # Truncated to ~50KB plus a small marker
    assert len(body.encode("utf-8")) <= (50 * 1024) + 200
    assert "truncated" in body
    # Findings still parsed correctly
    data = json.loads(j.read_text())
    assert data["findings"] == []
    assert data["score"] == 50
```

- [ ] **Step 2: Run tests**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py -v
```

Expected: 8 passed.

- [ ] **Step 3: Commit**

```bash
git add .github/scripts/test_extract_verdict.py
git commit -m "test(verdict-review): cover 50KB body truncation"
```

---

## Task 6: Extractor — empty findings goal state (TDD, regression-style)

**Files:**
- Modify: `.github/scripts/test_extract_verdict.py` (append 1 test)

This is the goal-state test — once iii reaches zero findings, this is what every nightly verdict looks like. Treating it as a regression check ensures we don't accidentally classify clean as broken.

- [ ] **Step 1: Add the failing test (case 7)**

Append to `.github/scripts/test_extract_verdict.py`:

```python


def test_empty_findings_goal_state(tmp_path):
    """Case 7: clean verdict (zero findings, score 100) — the goal state.

    This is a regression test: the extractor must NOT misclassify a clean
    verdict as missing-contract. An empty findings array is valid output.
    """
    body = "## Verdict\n\nClean. No concerns surfaced.\n"
    contract = '{"score":100,"findings":[]}'
    exec_path = _write_exec(tmp_path, body + "\n" + contract)

    _, md, j = _run(tmp_path, exec_path)

    data = json.loads(j.read_text())
    assert data["findings"] == []
    assert data["score"] == 100
    assert "Clean" in md.read_text()
    # Critical: must NOT have triggered the fallback
    assert "missing JSON contract" not in md.read_text()
    assert not any(
        "missing JSON contract" in (f.get("title") or "") for f in data["findings"]
    )
```

- [ ] **Step 2: Run tests**

```bash
python3 -m pytest .github/scripts/test_extract_verdict.py -v
```

Expected: 9 passed.

- [ ] **Step 3: Commit**

```bash
git add .github/scripts/test_extract_verdict.py
git commit -m "test(verdict-review): cover zero-findings goal state"
```

---

## Task 7: Workflow YAML — author the file

**Files:**
- Create: `.github/workflows/verdict-review.yml`

The YAML below is final per the eng review (D1=leak-count-only, D2=Slack failure ping, D3=nightly+Opus). Every line is intentional — do not "tidy up" without re-checking against the spec at `~/.claude/plans/verdict-i-d-treat-cheerful-starlight.md`.

- [ ] **Step 1: Create the workflow file**

Write to `.github/workflows/verdict-review.yml`:

```yaml
name: verdict-review

on:
  schedule:
    - cron: '0 7 * * *'   # 04:00 BRT / 07:00 UTC — after nightly traffic
  workflow_dispatch: {}

permissions:
  contents: read
  actions: read           # required for `gh run download` of prior artifact

concurrency:
  group: verdict-review
  cancel-in-progress: false

jobs:
  verdict:
    runs-on: ubuntu-latest
    timeout-minutes: 25
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 1

      - name: Load verdict prompt into env
        run: |
          {
            echo 'VERDICT_PROMPT<<PROMPT_EOF'
            cat .github/verdict-prompt.md
            echo 'PROMPT_EOF'
          } >> "$GITHUB_ENV"

      - name: Fetch previous run's findings count (best-effort)
        id: prev
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          # Find the most recent prior successful verdict-review run on main.
          # Tolerate missing/no-prior — first run reports "vs N/A".
          PREV_COUNT=""
          PREV_RUN=$(gh run list \
            --workflow verdict-review.yml \
            --branch main \
            --status success \
            --limit 2 \
            --json databaseId \
            -q '.[1].databaseId' 2>/dev/null || true)
          if [ -n "$PREV_RUN" ]; then
            mkdir -p /tmp/prev-verdict
            if gh run download "$PREV_RUN" \
                 --name "verdict-${PREV_RUN}" \
                 --dir /tmp/prev-verdict 2>/dev/null; then
              PREV_COUNT=$(jq -r '.findings | length' \
                /tmp/prev-verdict/verdict.json 2>/dev/null || echo "")
            fi
          fi
          echo "count=${PREV_COUNT}" >> "$GITHUB_OUTPUT"

      - name: Run verdict review
        id: claude
        uses: anthropics/claude-code-action@v1
        with:
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          prompt: ${{ env.VERDICT_PROMPT }}
          claude_args: >-
            --model claude-opus-4-7
            --max-turns 40
            --allowed-tools "Read,Glob,Grep,Bash(rg:*),Bash(fd:*),Bash(cargo metadata),Bash(cat:*),Bash(jq:*)"

      - name: Extract verdict markdown + findings JSON
        id: extract
        if: always()
        env:
          EXEC_FILE: ${{ steps.claude.outputs.execution_file }}
          PREV_COUNT: ${{ steps.prev.outputs.count }}
        run: |
          python3 .github/scripts/extract_verdict.py \
            "$EXEC_FILE" verdict.md verdict.json
          CURR=$(jq -r '.findings | length' verdict.json)
          echo "curr_count=$CURR" >> "$GITHUB_OUTPUT"
          if [ -n "$PREV_COUNT" ]; then
            DELTA=$((CURR - PREV_COUNT))
            if   [ "$DELTA" -lt 0 ]; then DELTA_STR="(${DELTA} vs yesterday)"
            elif [ "$DELTA" -gt 0 ]; then DELTA_STR="(+${DELTA} vs yesterday)"
            else                          DELTA_STR="(no change)"
            fi
          else
            DELTA_STR="(vs N/A — first run)"
          fi
          echo "delta_str=$DELTA_STR" >> "$GITHUB_OUTPUT"

      - name: Upload verdict artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: verdict-${{ github.run_id }}
          path: |
            verdict.md
            verdict.json
          retention-days: 90

      - name: Post verdict to Slack
        if: success()
        uses: slackapi/slack-github-action@v2.0.0
        with:
          method: chat.postMessage
          token: ${{ secrets.SLACK_BOT_TOKEN }}
          payload: |
            channel: ${{ secrets.SLACK_CHANNEL_ID }}
            text: "iii verdict — ${{ steps.extract.outputs.curr_count }} finding(s) ${{ steps.extract.outputs.delta_str }}"
            blocks:
              - type: header
                text:
                  type: plain_text
                  text: "iii nightly verdict — ${{ steps.extract.outputs.curr_count }} finding(s) ${{ steps.extract.outputs.delta_str }}"
              - type: section
                text:
                  type: mrkdwn
                  text: "<${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}|Open run> · artifact: `verdict-${{ github.run_id }}`"

      - name: Post failure ping to Slack
        if: failure()
        uses: slackapi/slack-github-action@v2.0.0
        with:
          method: chat.postMessage
          token: ${{ secrets.SLACK_BOT_TOKEN }}
          payload: |
            channel: ${{ secrets.SLACK_CHANNEL_ID }}
            text: "iii nightly verdict run FAILED"
            blocks:
              - type: header
                text:
                  type: plain_text
                  text: "iii nightly verdict — run FAILED"
              - type: section
                text:
                  type: mrkdwn
                  text: "Workflow failed before producing a verdict.\n<${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}|Open run for logs>"
```

- [ ] **Step 2: Sanity-check the file shape**

```bash
wc -l .github/workflows/verdict-review.yml
grep -c "uses:" .github/workflows/verdict-review.yml
```

Expected: ~115 lines; 4 `uses:` references (`checkout@v4`, `claude-code-action@v1`, `upload-artifact@v4`, two of `slack-github-action@v2.0.0`).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/verdict-review.yml
git commit -m "feat(verdict-review): add nightly workflow"
```

---

## Task 8: Validate the workflow with actionlint

**Files:**
- (no edits — validation only, fix any flagged issues inline)

- [ ] **Step 1: Run actionlint**

```bash
actionlint .github/workflows/verdict-review.yml
```

Expected: zero output, exit 0.

- [ ] **Step 2: If actionlint flags anything, fix it**

Common false positives and real issues to expect:
- **Shellcheck SC2086** on `${{ steps.prev.outputs.count }}` interpolation — actionlint may suggest quoting; the env var passthrough already handles this. If flagged, add `# shellcheck disable=SC2086` only if the suggested quoting breaks behavior.
- **Unknown action version** — confirm `slackapi/slack-github-action@v2.0.0` is the version used in `.github/workflows/_homebrew.yml`. Run `grep -r 'slackapi/slack-github-action' .github/workflows/` to verify.
- **Missing input** on `claude-code-action@v1` — if actionlint complains about `claude_args` or `prompt`, run `gh api repos/anthropics/claude-code-action/contents/action.yml -H 'Accept: application/vnd.github.raw' | grep -A2 '^[a-z_]*:'` to verify input names against the published action.

Fix any real issues directly in the YAML, then re-run actionlint.

- [ ] **Step 3: If you fixed anything, commit the fix**

```bash
# Only if you made changes
git add .github/workflows/verdict-review.yml
git commit -m "fix(verdict-review): address actionlint findings"
```

If no changes were needed, skip the commit.

---

## Task 9: Update WORKFLOWS.md

**Files:**
- Modify: `.github/workflows/WORKFLOWS.md`

The existing doc has three things to add to: the workflow table at the top, a dedicated `## verdict-review.yml` section, and the Secrets table. Match the existing style exactly.

- [ ] **Step 1: Read the current WORKFLOWS.md to find the insertion points**

```bash
grep -n "^### \`" .github/workflows/WORKFLOWS.md
grep -n "^## Secrets" .github/workflows/WORKFLOWS.md
grep -n "license-check" .github/workflows/WORKFLOWS.md
```

Note the line numbers for: (a) the last top-level workflow row in the table, (b) the `### \`license-check.yml\`` section heading (we insert the new section after it), (c) the Secrets table.

- [ ] **Step 2: Add a row to the workflow table at the top of the file**

Find the `## Top-Level Workflows` table (it's a markdown bullet list of triggers, not a real table — verify in the file). Add this entry in alphabetical order:

```markdown
### `verdict-review.yml` — Nightly AI Architectural Verdict

**Triggers:** `schedule` (cron `0 7 * * *`), `workflow_dispatch`

Runs `anthropics/claude-code-action@v1` (Opus 4.7) against a fresh checkout of the repo, parses the resulting verdict into `verdict.md` + `verdict.json`, uploads them as a 90-day artifact, and posts a Slack message to the release channel with the finding count and a day-over-day delta. Notify-only — never fails CI.

**Inputs:** none. The verdict prompt lives at [`.github/verdict-prompt.md`](../verdict-prompt.md) — edit it there to tune the brief without touching YAML.

**Failure path:** an `if: failure()` step posts an explicit "verdict run FAILED" ping to the same Slack channel so silent days can't hide drift.

**Cost:** estimated $5–$20 per run, $150–$600/month at nightly cadence. Monitor via the Anthropic API usage page.

---
```

Insert this section between `### \`license-check.yml\`` and the next `## Reusable Workflows` heading. Verify with:

```bash
grep -n "^### \`license-check\|^## Reusable" .github/workflows/WORKFLOWS.md
```

- [ ] **Step 3: Add `ANTHROPIC_API_KEY` to the Secrets table**

Find the secrets table (search for `SLACK_BOT_TOKEN`) and add:

```markdown
| `ANTHROPIC_API_KEY` | `verdict-review.yml` — Claude Code Action authentication |
```

Match the existing column shape (the existing rows are pipe-separated `secret | description` — copy the exact format from the line above `SLACK_BOT_TOKEN`).

- [ ] **Step 4: Verify**

```bash
grep -c "verdict-review" .github/workflows/WORKFLOWS.md
grep -c "ANTHROPIC_API_KEY" .github/workflows/WORKFLOWS.md
```

Expected: both ≥ 2 (section heading + secrets row + any internal references).

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/WORKFLOWS.md
git commit -m "docs(verdict-review): document workflow + secret in WORKFLOWS.md"
```

---

## Task 10: Pre-merge verification

**Files:** none (operator-driven validation against GitHub Actions + Slack)

This task is a **manual checklist** — every item must be verified before opening the PR for merge.

- [ ] **Step 1: Run the extractor test suite from a clean state**

```bash
python3 -m pytest .github/scripts/ -v
```

Expected: 9 passed, 0 failed, 0 errors.

- [ ] **Step 2: Run actionlint one more time**

```bash
actionlint .github/workflows/verdict-review.yml
```

Expected: zero output, exit 0.

- [ ] **Step 3: Add the `ANTHROPIC_API_KEY` repo secret**

```bash
# Operator types this manually with a real key from console.anthropic.com:
gh secret set ANTHROPIC_API_KEY --repo iii-hq/iii
```

Or via the web UI: Settings → Secrets and variables → Actions → New repository secret.

Verify it exists:

```bash
gh secret list --repo iii-hq/iii | grep ANTHROPIC_API_KEY
```

- [ ] **Step 4: Push the branch and open a draft PR**

```bash
git push -u origin feat/verdict-review-workflow
gh pr create --draft \
  --title "feat(ci): nightly AI verdict review workflow" \
  --body "$(cat <<'EOF'
## Summary

Adds a nightly GitHub Actions workflow that runs `anthropics/claude-code-action@v1` against a fresh checkout of iii, parses the resulting verdict, and posts it to the existing release Slack channel with a day-over-day finding count delta.

## Test plan

- [ ] `python3 -m pytest .github/scripts/` passes (9 tests)
- [ ] `actionlint .github/workflows/verdict-review.yml` clean
- [ ] `workflow_dispatch` dry-run posts a verdict to Slack
- [ ] `workflow_dispatch` with bad API key posts a "FAILED" ping to Slack
- [ ] First scheduled run shows `(vs N/A — first run)`; second shows a delta
EOF
)"
```

- [ ] **Step 5: Success-path dry-run**

Trigger the workflow manually:

```bash
gh workflow run verdict-review.yml --ref feat/verdict-review-workflow
```

Watch it:

```bash
sleep 10
gh run list --workflow verdict-review.yml --limit 1
gh run watch $(gh run list --workflow verdict-review.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

**Confirm:**
- The workflow finishes in under 25 minutes.
- An artifact `verdict-<run_id>` exists with `verdict.md` and `verdict.json` inside.
- A Slack message appears in the release channel with header `iii nightly verdict — N finding(s) (vs N/A — first run)` and a link to the run.
- `verdict.md` is readable and contains the four required sections.

If the verdict surfaces real findings (likely on first run), capture them — these become the first round of follow-up commits to drive findings_count toward zero.

- [ ] **Step 6: Failure-path dry-run**

Temporarily override `ANTHROPIC_API_KEY` to a bogus value and trigger again:

```bash
gh secret set ANTHROPIC_API_KEY --repo iii-hq/iii --body "sk-bogus-for-testing"
gh workflow run verdict-review.yml --ref feat/verdict-review-workflow
gh run watch $(gh run list --workflow verdict-review.yml --limit 1 --json databaseId -q '.[0].databaseId') || true
```

**Confirm:**
- Slack receives `iii nightly verdict — run FAILED` with a link to the run logs.

**Restore the real key:**

```bash
gh secret set ANTHROPIC_API_KEY --repo iii-hq/iii  # paste the real key when prompted
```

- [ ] **Step 7: Mark PR ready, request review, merge**

```bash
gh pr ready
gh pr merge --squash --auto  # or merge manually after CI green
```

- [ ] **Step 8: Confirm production behavior after merge**

After merging to main, trigger one final manual run on main to confirm the schedule will pick it up cleanly:

```bash
gh workflow run verdict-review.yml --ref main
```

Then let the cron take over the next morning. Track `findings_count` in the Slack header day over day. **Goal state:** header reads `iii nightly verdict — 0 finding(s) (no change)` for 7 consecutive nights.

---

## Self-Review Checklist (run before handoff)

After completing all 10 tasks, verify against the spec at `~/.claude/plans/verdict-i-d-treat-cheerful-starlight.md`:

- [ ] All 5 files in the "Files touched" table exist and contain the expected content
- [ ] All 8 decisions from the spec's decision table are reflected in the implementation (cadence, reviewer, failure mode, channel, model context, trend signal, action-failure path, cost)
- [ ] All 6 failure modes in the spec's table have either a test or an explicit handler in the YAML
- [ ] No `ANTHROPIC_API_KEY` value committed to the repo (only the secret name in YAML/docs)
- [ ] The 8 "NOT in scope" items from the spec are not accidentally implemented
- [ ] Both Slack steps (`if: success()` and `if: failure()`) reference the same `SLACK_CHANNEL_ID` secret
- [ ] No `id-token: write` permission anywhere
- [ ] The verdict prompt explicitly contains the phrase "you have no memory of any prior reviews"

If any item fails, open the relevant task and fix it inline before requesting review.

---

## Out of scope (deferred to follow-on PRs)

These are explicitly **not** in this plan, per the eng review:

| Out of scope | Why |
|--------------|-----|
| Per-PR verdict comments | Cost + noise; nightly is the agreed cadence |
| Auto-opening GitHub issues per finding | Notify-only mode; humans triage |
| Cross-run trend dashboards | Would require persistent state beyond the one-integer delta |
| Codex / GPT second opinion | Single reviewer for v1 |
| Failing CI on findings | Notify-only — pressure comes from Slack visibility |
| `actionlint` gate in `ci.yml` | Deferred — keeps this PR additive |
| Extractor tests in `ci.yml` | Deferred — same reason |
| `CLAUDE.md` for richer model context | Deferred — prompt-only briefing is sufficient for v1 |
