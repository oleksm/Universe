# Agent messaging: agentmsg

For installation and daily operation, start with the [operator README](../tools/agents/README.md).
The [operating skill](../tools/agents/skills/agentmsg/SKILL.md) teaches this workflow to Claude and Codex.

A mailbox any agent with a shell can use: Claude Code sessions, Codex, scripts. Files, not sockets, so it works across
tools, survives restarts and power cuts, and costs no tokens while an agent waits. `tools/agents/agentmsg.py`, linked as
`~/bin/agentmsg`; the mailbox is `~/.agents` (or `$AGENTMSG_HOME`).

## Commands

    agentmsg send <to> <kind> --summary "..." [--ref KEY] [--see PATH]... [--reply-to ID] [--as ME]
    agentmsg inbox [--as ME] [--peek] [--quiet]     unread summaries, one line each; marks them read
    agentmsg show <id>                               one message in full
    agentmsg wait [--as ME] [--timeout S]            block until mail arrives (inotify), then print it
    agentmsg serve                                   watch every inbox: wake or notify the agent mail is for
    agentmsg open [--as ME]                          what I owe and what I wait on, a few lines
    agentmsg who                                     agents, their unread counts, auto or notify

`kind` is one of task, ack, done, blocked, note. A summary is at most 400 characters: detail lives in files, commits
and registry keys, pointed at with `--ref` and `--see`, and the receiver opens them only if it needs them. A hand-off is
a `task`, answered by `done` (with a commit or a path) or `blocked` (one line why), `--reply-to` the task's id; an
`ack` in between is optional.

## Spending few tokens

The message is cheap (about 60 tokens); the wake is not: a turn re-reads the agent's whole conversation (engine's was
622k tokens a call on 2026-10-10). So the rules cut wakes, not words:

- **Answer only a task, a blocked or a question** (a summary with `?`). The delivered text gives the finished reply command
  for each, says to start on it (after the current task, if busy), to ask other agents by mail when blocked or in need
  of help, and to send results by mail, not only in the window; the rest needs no reply. Never answer an ack, note or done.
- **Acks never wake anyone.** They stay unread and come with the next mail or turn that does wake the agent.
- **One wake per burst.** `serve` delivers an inbox once it has been quiet for 20 s (`AGENTMSG_SETTLE`), all of it at once.
- **Status without scrolling:** `agentmsg open` lists tasks and blocks to me with no `done`/`blocked` from me on their
  thread (owe) and my tasks with none back (waiting).
- A long conversation makes every wake dear: compact it (`/compact`) at a quiet moment.

## Who is who: `~/.agents/agents.json`

    {"registry": {"cwd": "~/git/universe-fso"},
     "blender":  {"cwd": "~/git/blender", "wake": ["codex", "exec", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox", "{prompt}"]}}

An agent is known by `--as`, else `$AGENTMSG_NAME`, else the entry whose `cwd` holds the current directory.

## Launching everyone: `agents-up`

    agents-up                    # every agent with a start command: started if down, a window on each nobody watches
    agents-up engine blender     # only these
    agents-up --no-windows       # started, no windows

It is `agentmsg up`. What is up is left alone, so run it whenever: after a reboot, after a window closed, when unsure.
An agent whose conversation is open in another terminal is skipped with its pid (close it there first). Each agent's
start command is its `term` in agents.json, set once with `agentmsg term <name> -- <command>`.

## Watching an agent and letting it answer: `agentmsg term`

    agentmsg term blender -- codex --dangerously-bypass-approvals-and-sandbox    # first time: how it starts
    agentmsg term blender                                                        # later: start it, or open a window on it
    agentmsg term blender --here                                                 # in this terminal instead of a new window

Each agent opens in its own terminal window, so starting a second never lands in the first one's window. Claude
sessions of several agents share one project folder (~/git/universe), where `claude --continue` picks whichever
session ran last: so a Claude agent's command resumes its own session by id, `sh -c "cd ~/git/universe && exec claude
--resume <id>"` (the id is the session's file name under ~/.claude/projects/; `/rename` titles show which is which). Put the id in agents.json as `"session": "<id>"` too: the Stop hook
then knows the session by its id, not its folder, and a session that is not the folder's agent takes no mail.

The agent runs in a tmux session, `agent-<name>`, on its own tmux server (socket `agent-<name>`: one going down takes no other with it), in its folder, so you watch it and type into it like any session;
detach with Ctrl-b d and it keeps running. While its session is up, `serve` pastes each piece of mail into it as one
line (a bracketed paste, so the TUI does not take its Enter as a new line), presses Enter, and presses it again (up to
three times) if the line still sits in the input box (Codex and Claude take it as your next message, queued if mid-turn), and sends a desktop
notification. Its replies come back through `agentmsg send`. `agentmsg who` shows `term` for an agent running this way.

## Being told

- **A Claude Code session** has a Stop hook (`~/.claude/settings.json`: `agentmsg hook-claude`): when the session is
  about to end its turn and its agent has unread mail, the hook keeps it going with the summaries.
- **An idle agent**: `agentmsg serve` watches the inboxes. It runs as a user service, `agentmsg.service`
  (`~/.config/systemd/user/`; `systemctl --user status agentmsg`), started at login and restarted if it stops. For an
  agent with a `wake` command it starts that command with the mail as the prompt (`{prompt}`), one run at a time,
  logging to `~/.agents/log/<agent>.log`; the agent reads, works and replies with `agentmsg send`. For an agent without
  one it sends a desktop notification. Wake is opt-in per agent: a Codex agent wakes with `codex exec … "{prompt}"`
  (or `codex exec resume <session> "{prompt}"`); a Claude agent with `claude -p --resume <session> "{prompt}"` plus the
  permission flags it needs.
- **An agent waiting on a reply** runs `agentmsg wait` in the background: it returns when the mail arrives.

## Tested

2026-10-10: a task sent to a Codex agent with a wake command was answered (`done … pong`) in 7 seconds through
`serve`; the Stop hook, `wait`, `inbox --quiet` and `who` checked by hand.

## Portable agents: onboard, switch, inspect, test

An agent name is permanent; Claude and Codex are runtimes for that name. Keep the
mailbox, working directory and task threads when switching runtimes. These commands
are implemented in `tools/agents/agentnet.py`, next to `agentmsg.py`.

### Onboard and launch

```sh
agentmsg register designer --cwd ~/git/my-project --runtime codex
agentmsg launch designer
agentmsg launch designer --no-windows
```

`launch` starts the configured user service, sets `AGENTMSG_NAME` and
`AGENTMSG_HOME`, and opens a managed tmux terminal. Its initial prompt supplies the
network instructions. A new Codex agent runs `agentmsg bind designer --runtime codex`
from its own session; this reads `CODEX_THREAD_ID` (or `CODEX_SESSION_ID`) and saves
an exact resume command. New Claude sessions receive an explicit UUID at launch.
To bind an existing session explicitly:

```sh
agentmsg bind designer --runtime claude --session UUID
```

`bind` records a session; it does not launch it or terminate another conversation.
An existing legacy resume command is kept by `launch`, including its flags and cwd
wrapper; direct Codex commands gain `--no-daemon` unless explicitly remote. `runtime_args` in an agent's config can store argument arrays for each
runtime, e.g. `{"codex": ["--sandbox", "workspace-write"], "claude": []}`. New Codex
sessions use `--no-daemon` to avoid inheriting a different agent's shell environment
from the shared Codex daemon, while retaining the selected logical agent identity.
They default to workspace-write with the mailbox directory added as a writable
root. Claude gets the mailbox directory as an additional directory. Normal tool
permission prompts still apply. Direct CLI flags preceding a known resume command
are saved when switching away; complex shell wrappers are retained in the switch
backup, not translated into the other provider's arguments.

### Switch with the current context

```sh
agentmsg switch engine --to claude
agentmsg switch engine --to codex
```

This requests a handoff and waits up to 180 seconds (override with `--wait`). The
outgoing agent writes `CONTEXT.md` describing the objective, constraints, decisions,
files, commits, validation, unfinished work and outstanding communication. It then
runs the provided `agentmsg handoff-ready PATH` command and stops work. Delivery to
that identity pauses until the switch completes. A safe stopping point is required;
the launcher does not infer that an agent is idle from its process existing.

The switch replaces the source CLI inside the same managed tmux session only
after its explicit ready signal. The attached terminal window stays open; the new
conversation reads the handoff path and keeps the same mailbox. Work done in either runtime therefore travels into the next handoff.
Provider-private session databases are never rewritten. Installed Claude Code
2.1.284 exposes configuration import, not cross-provider conversation import;
Codex 0.163.0 exposes resume/fork, not Claude history import. This implementation
uses context handoff rather than fabricating native session records.

If the agent is busy or only has notification delivery, the command prints a
continuation command after the wait. Run that command when its context is ready:

```sh
agentmsg switch engine --to claude --handoff /path/printed/by/the/tool
```

For separate preparation:

```sh
agentmsg spool engine
agentmsg switch engine --to claude --handoff /path/to/handoff --no-start
agentmsg launch engine
```

`--no-start` changes the configured runtime after the safe stopping checks but
leaves launch for later. `--no-windows` starts the managed terminal without opening
a desktop window. `agentmsg handoff-cancel engine` resumes delivery and invalidates
a ready handoff if you decide to keep working in the source. A used handoff cannot
be reused: spool the current state for the next switch.

When the source has exhausted its quota, use the offline fallback:

```sh
agentmsg spool engine --snapshot
# Exit the source session, then:
agentmsg switch engine --to claude --handoff /path/to/snapshot
```

A snapshot needs no model turn. It saves recorded conversation text/tool records,
mail history and Git status/log/diff statistics, with a clearly labelled automatic
context note. An optional `--source /path/to/session.jsonl` selects the source log.
Without an exact registered session (for example a legacy `resume --last` entry),
no transcript is guessed: the manifest reports zero exported records. Bind the
session or supply its log when transcript recovery is needed. The original session
files and working tree are retained. The snapshot does not capture unsaved editor
buffers, a process's memory, or history absent from the source JSONL. It does not
invent a semantic summary. The destination must inspect the recorded context.

If the source is running outside the managed terminal and its session ID is visible
in its process command, switching asks you to exit that conversation before
continuing. Processes whose session identity is not observable cannot be reliably
detected; close those manually before switching. A snapshot never kills a live
managed source without an agent-written ready signal.

Handoffs live under `~/.agents/handoffs/NAME/`; switch events and the previous config
are retained under `~/.agents/switches/`. They contain local project context. The
internal `_agentmsg` mailbox holds orchestration replies and does not wake an LLM
or trigger desktop notifications.

### Test both directions of a connection

```sh
agentmsg probe engine ships --wait 60
agentmsg probe --check probe-ID --wait 60
```

The probe sends a task to each agent. Each must execute `probe-send` to send a ping
to its peer; each peer must execute `probe-ack` with the actual ping message ID.
`PASS` requires **both** pings and **both** matching acknowledgements, with the
correct sender, recipient, nonce and reply thread. Reading an inbox, a running
process, a generic `done` reply, or one-way delivery cannot produce a pass. Steps
are idempotent. A probe expires after one hour; a wait timeout is `PENDING`, not a
claim that the agents are disconnected. Agents can be busy, quota-limited or
waiting for a permission prompt. Inspect `status`, the terminal, or wake logs.

A probe is a live agent test and uses model turns. Ordinary `ack` messages still
never wake an agent. The acknowledgement helper also closes the ping task with a
`done` message, so the probe does not leave outstanding tasks behind.

### Inspect status and usage

```sh
agentmsg status
agentmsg status --usage
agentmsg status --usage --json
```

Status reports runtime/session, delivery route, unread/owed/waiting counts, message
counts, outgoing peers, last sent time, handoff pause state, probe outcomes and
switch history. Sent and received copies are deduplicated by message ID. Delivery
availability is not a responsiveness claim: use a probe for that.

`--usage` reads the registered current session's recorded usage, not account billing
or a complete history across every provider session. Claude API response IDs are
deduplicated; Codex's latest cumulative token totals are used rather than adding
cumulative snapshots together. Codex quota observations include their recorded
timestamp and may be stale. Missing usage, session identity, or quota data is
reported as null/unknown. No costs or remaining quotas are inferred from token
counts. Old handoffs/switch events retain source-session references for inspection.

### Validate changes

```sh
python -m unittest discover -s tools/agents -p 'test_*.py' -v
```

Tests use an isolated mailbox. For live testing, use `AGENTMSG_HOME` and disposable
agents rather than consuming project agents' outstanding tasks.

### Canonical names and compatibility aliases

Use role names (`ships`, `engine`) independently of the runtime. `aliases.json` in
`AGENTMSG_HOME` maps older names to canonical ones, for example:

```json
{"ships-claude": "ships", "engine-claude": "engine"}
```

Sending, reading, explicit `--as`, inherited `AGENTMSG_NAME`, binding and launcher
commands resolve aliases. Historical message IDs and reply threads stay intact;
status normalizes the endpoints for counts. When consolidating existing agents,
back up their configuration and merge their `new`, `read` and `sent` mail folders
into the canonical mailbox; aliases route future messages but do not move existing
files by themselves. Keep the actual current conversation's runtime/session in the
canonical configuration, rather than an older runtime's saved session.

A renamed tmux session can keep its original server socket and attached window.
Set `tmux_socket` in the canonical agent entry to that socket name. For example,
`ships` can use session `agent-ships` on socket `agent-ships-claude`; the visible
session name is `agent-ships` and subsequent launches/attachments use that mapping.
