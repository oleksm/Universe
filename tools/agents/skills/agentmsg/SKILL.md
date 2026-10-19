---
name: agentmsg
description: >-
  Operate the local agentmsg network: onboard or reopen agents, switch an agent
  between Claude Code and Codex with its context intact, diagnose delivery, run
  real ping/ack tests, and inspect agent usage. Use for agentmsg operations or
  requests to reconnect or switch an existing network agent.
---

# Operate agentmsg

Keep the agent's role identity, mailbox, conversation context and pending decisions
across runtime changes. `ships` remains `ships` on Claude or Codex; its managed
terminal is `agent-ships`. Reopening an agent should resume its saved conversation.

The operator guide is [../../README.md](../../README.md). For detailed behavior,
locate the installed executable with `readlink -f "$(command -v agentmsg)"`; its
adjacent README links to the repository's `docs/agent-messaging.md`. Use installed
`--help` when command availability differs. Do not fabricate unsupported commands.

## Establish the correct identity

- Inspect `agentmsg status --json`, `${AGENTMSG_HOME:-$HOME/.agents}/agents.json`
  and, when needed, `aliases.json`. Distinguish the agent name from its runtime,
  native session UUID and tmux session name.
- Inspect other agents' mail with `inbox --as NAME --peek`; ordinary `inbox` claims
  it. Check your own inbox and open threads at the start and end of work when you
  have an established identity. A “who are you?” error from a neutral directory
  does not justify choosing another agent's identity for unrelated work.
- Directory matches are insufficient when several conversations share a folder.
  `claude agents --json` identifies live Claude names, UUIDs, PIDs and busy/idle
  state in supported versions. Compare these with configured exact session IDs.
- Do not create a parallel `NAME-codex` or `NAME-claude` identity when the user wants
  to continue the same agent. Old provider-suffixed names may already be aliases.

## Reopen, onboard or switch

**Reopen:** `agentmsg launch NAME`. Confirm its saved conversation and attached
managed terminal exist. `status` reports delivery availability; it does not prove a
fresh response. Do not replace its history with a new session just to open a window.

**New agent:** `agentmsg register NAME --cwd PATH --runtime codex` (or `claude`),
then `agentmsg launch NAME`. Codex's bootstrap binds its actual `CODEX_THREAD_ID`;
from an operator shell supply an explicit UUID when binding. Never bind an agent
to the operator's own session ID accidentally.

**Runtime switch:** `agentmsg switch NAME --to codex` (or `claude`). The source
writes context and calls `handoff-ready`; the launcher replaces it in the managed
terminal and the replacement reads the handoff. Preserve unresolved user decisions
as unresolved. Prefer this to editing private provider session databases.

If switching times out, retain and use the printed `--handoff PATH` continuation.
Do not repeatedly request new spools. A busy source gets time to reach a safe
stopping point. A quota-exhausted source can use `spool NAME --snapshot`, followed
by switching from that snapshot after the source exits. Report that this is recovery
from recorded context, not an agent-written summary. Verify a transcript source
was found; use exact binding or `--source` when needed.

An unmanaged idle terminal with notification-only delivery will not automatically
read a new task. For a requested reconnection or switch, identify its exact session
and safely move that conversation under `launch`. Preserve the source conversation
and working tree. If stopping a process is necessary, verify its session, PID,
process start identity and safe idle/stopping state immediately beforehand. Do not
kill an arbitrary process found by a broad name search or run duplicate sessions.
If those facts cannot be established, explain the specific missing fact and ask
for the minimum intervention needed.

New managed Codex launches use `--no-daemon` so another agent's shell environment
does not leak into them. This preserves the selected logical identity. Use explicit
`AGENTMSG_NAME`/`AGENTMSG_HOME` or `--as` when shell startup may alter the environment.

## Verify actual communication

For a requested live connection test:

```sh
agentmsg probe ships engine --wait 60
agentmsg probe --check probe-ID --wait 60
```

Let the actual agents execute `probe-send` and `probe-ack`. Never execute their
steps on their behalf and call that an agent connectivity pass. `PASS` requires two
pings and two matching peer acknowledgements. A generic completion reply or one-way
receipt is not equivalent. Recheck the same probe while agents finish work; avoid
repeated wakes. An old passing probe is historical evidence, not a fresh test.

If no reply arrives, inspect the watcher service, canonical name, configured native
session, delivery route and target terminal. A stale Codex session pointer can
coexist with the live Claude conversation of the same role. Correct that routing
instead of merely resending the ping. `notification` is not connected terminal
delivery; `terminal` is not proof the model has processed its queue.

Send control messages necessary for the user's requested operation or test. Do not
use network maintenance as permission to assign unrelated project work. Message
summaries are at most 400 characters; put details in files with `--see`. Replies
use `--reply-to`, with `done` naming results or `blocked` explaining the obstacle.
Do not answer routine acknowledgements, notes or completion messages.

## Names, status and maintenance

When the user requests a rename/consolidation, preserve current runtime/session,
mail, thread IDs and the attached terminal. Back up configuration and mailbox
files first. Map obsolete names through `aliases.json`; aliases alone do not move
existing mail. Merge old mail without reintroducing read messages as unread. A
renamed tmux session may keep its existing socket through `tmux_socket`. Use the
operator guide for the exact storage layout. Do not invent an automatic rename CLI.

Use `status --usage --json` for recorded session tokens, timestamped quota
observations, peers and work counts. Missing data is unknown; do not infer account
billing or remaining quota from tokens. Avoid copying private transcripts, live
mailboxes, account details or local configuration into public repository docs.

When changing the tool, run its isolated unittest suite. Run live probes only when
the task calls for them, using disposable agents for implementation experiments.
Report the concrete result, current canonical identity/runtime, and any unresolved
connection or handoff step.
