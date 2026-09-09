# Muse Spark on a ChatGPT parent

This branch (`rust-v0.153.4-muse`, from tag `rust-v0.153.4`) lets a ChatGPT-authenticated Codex parent spawn a child role that talks to Meta Muse Spark through a local [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) instance. Default OpenAI / ChatGPT behavior is unchanged.

Upstream Codex still ignores `model_provider` on agent roles ([openai/codex#40858](https://github.com/openai/codex/issues/40858)). This fork adds that override, rewrites encrypted `agent_message` / custom-tool items for non-OpenAI providers, and accepts `apply_patch` as a function call with an `input` string.

Machine-local files stay off this repo: no API keys, no `~/.cli-proxy-api/config.yaml`, no `~/.codex/config.toml`, no session logs.

## Parent stays on ChatGPT

Log the parent in with ChatGPT as usual. Do **not** set a top-level `model_provider` in `~/.codex/config.toml`. The parent keeps using OpenAI; only the Muse role switches provider.

Add a provider the role can select (the name must already exist on the parent):

```toml
[model_providers.cliproxy]
name = "CLIProxyAPI"
base_url = "http://127.0.0.1:8317/v1"
wire_api = "responses"
env_key = "CLIPROXY_API_KEY"
requires_openai_auth = false
```

`CLIPROXY_API_KEY` is whatever local key your CLIProxyAPI `api-keys` list accepts. Put the real value in the environment or a local secrets file, not in git.

## Role file

Save as `~/.codex/agents/muse-spark.toml` (the role name is `muse_spark`):

```toml
name = "muse_spark"
description = "Cheap Muse Spark 1.3 worker via local CLIProxyAPI. Spawn with a task_name that starts with muse_. Never pass model=muse-spark-1.3. Avoid fork_turns=all; pass a positive integer of recent parent turns."
nickname_candidates = ["Muse", "Spark"]
model = "muse-spark-1.3"
model_provider = "cliproxy"
model_reasoning_effort = "max"
model_reasoning_summary = "auto"
model_context_window = 372000
model_auto_compact_token_limit = 334800
developer_instructions = """
You are a Muse Spark 1.3 worker for a smarter parent agent. Do simple, concrete grunt work: search, list, read, tally, and report. Do not make architecture calls or claim the work is finished or verified. Return a self-contained result the parent can check.

If a task header says the encrypted payload was omitted, the assignment is in the parent conversation above that header. Follow those recent parent turns.
"""

[features]
apps = false
plugins = false
web_search_request = false
web_search_cached = false
standalone_web_search = false
```

`model_provider` may only name a provider already defined on the parent (see above). Roles cannot invent endpoints.

## How to spawn

Use native multi-agent v2 `spawn_agent` with:

- `task_name` starting with `muse_` (for example `muse_inventory`). That prefix selects `muse_spark` and routes the child to `cliproxy`.
- Do **not** pass `model=muse-spark-1.3`. That slug is not a ChatGPT model; the parent backend rejects it. This fork also drops that argument if it is sent.
- Set `fork_turns` to a positive integer of recent parent turns. `fork_turns=all` (or omitting it) is capped to the last 30 turns for Muse.
- Put the actual assignment in those recent parent turns as well as in the spawn message. ChatGPT encrypts the spawn `message`; the local provider cannot read it, so the forked parent context is the task.

Tell the parent the same rules in `~/.codex/AGENTS.md` if you want it to spawn Muse without extra prompting.

## CLIProxyAPI

Point a `codex-api-key` entry at Meta’s Responses API and enable multi-agent v2 helpers. Example shape only — use your own key and leave this file on the machine:

```yaml
host: "127.0.0.1"
port: 8317

codex:
  optimize-multi-agent-v2: true

codex-api-key:
  - api-key: "YOUR_META_API_KEY"
    base-url: "https://api.meta.ai/v1"
    models:
      - name: "muse-spark-1.3-contributor"
        alias: "muse-spark-1.3"
        display-name: "Muse Spark 1.3"
        max-context-length: 372000
        force-mapping: true
        is-compat: true
```

`optimize-multi-agent-v2` helps when traffic already reaches the proxy. It does not replace this Codex patch: a ChatGPT-authenticated parent never sends the child to cliproxy unless the role can set `model_provider`.

Do not also register the same `muse-spark-1.3` alias on an `openai-compatibility` Chat Completions route, or requests will round-robin and lose Responses behavior.

## Build and install

```bash
git clone -b rust-v0.153.4-muse https://github.com/aytekaksu/codex.git
cd codex/codex-rs
cargo build --release -p codex-cli
```

The binary is `codex-rs/target/release/codex`.

- CLI: copy it onto your `PATH`, or to `~/.codex/bin/codex`.
- Codex Desktop on macOS: replace `/Applications/ChatGPT.app/Contents/Resources/codex` with the new binary (the app may need to be quit first; replacing the bundled binary often needs an admin copy). Ad-hoc sign after replacing:

```bash
codesign -s - --force --timestamp=none /Applications/ChatGPT.app/Contents/Resources/codex
```

Keep a copy of the stock signed binary before overwriting it.

To pick up a newer official `rust-v*` tag later: `git fetch upstream` and rebase this branch onto that tag. Do not open a PR to `openai/codex` unless you intend to.
