# agentmsg

Operate a network of persistent agents across Claude Code and Codex. An agent keeps
its name, mailbox and working context when its runtime changes. `ships` stays
`ships`; its terminal session stays `agent-ships`.

Messages are local JSON files under `~/.agents` (override with `AGENTMSG_HOME`).
A watcher delivers them to managed terminals, runs an explicitly configured wake
command, or sends a desktop notification. A notification alone does not wake an
idle agent. No hosted message service or Python packages are required.

## Everyday commands

```sh
agentmsg launch ships                    # start or reopen the saved conversation
agentmsg switch ships --to claude        # spool context, then replace the runtime
agentmsg switch ships --to codex
agentmsg probe ships engine --wait 60    # real two-way agent ping/ack test
agentmsg status                          # runtime, delivery, work and peer counts
agentmsg status --usage                  # recorded current-session tokens/limits
agentmsg status --usage --json           # machine-readable details
```

If you close an agent, run `agentmsg launch NAME` again. It resumes the configured
conversation, or attaches to it if still running. To leave it running while hiding
its terminal, detach with **Ctrl-b d**. `--no-windows` starts an agent without opening
a desktop window. `agents-up ships engine` starts both; `agents-up` starts all
configured agents with launch commands.

## Install on a Linux workstation

Requirements: Python 3.9+, tmux, and the Claude/Codex CLIs you intend to use, already
signed in. The managed launcher uses a systemd user scope when available. Install
`inotifywait` for efficient mailbox waiting; polling is the fallback. A terminal
launcher (`$TERMINAL`, Alacritty, Kitty, Foot, GNOME Terminal or xterm) is needed to
open windows. `notify-send` is optional.

From the repository root:

```sh
agentmsg_repo="$PWD"
mkdir -p "$HOME/.local/bin" "$HOME/.agents" "$HOME/.config/systemd/user"
ln -s "$agentmsg_repo/tools/agents/agentmsg.py" "$HOME/.local/bin/agentmsg"
ln -s "$agentmsg_repo/tools/agents/agents-up" "$HOME/.local/bin/agents-up"
export PATH="$HOME/.local/bin:$PATH"
```

If a link already exists, inspect it before replacing it. Keep `agentmsg.py` and
`agentnet.py` together; the command resolves its symlink to find the adjacent module.

Create `~/.config/systemd/user/agentmsg.service`:

```ini
[Unit]
Description=agentmsg mailbox watcher

[Service]
ExecStart=%h/.local/bin/agentmsg serve
Restart=always
RestartSec=5
Environment=PATH=%h/.local/bin:%h/bin:/usr/local/bin:/usr/bin

[Install]
WantedBy=default.target
```

```sh
systemctl --user daemon-reload
systemctl --user enable --now agentmsg.service
```

If configuring noninteractive `wake` commands, include their CLI directories in the
service's PATH, or use absolute executable paths. Alternatively, run `agentmsg
serve` in a dedicated shell. Use one watcher per mailbox. This service starts with
the user session; unattended operation after logout depends on the host's systemd
user-service setup.

## Onboard an agent

```sh
agentmsg register ships --cwd ~/git/my-ships-project --runtime codex
agentmsg launch ships
```

Registration saves the working directory and runtime in `~/.agents/agents.json`.
Launch supplies network instructions and sets the identity and mailbox environment.
Fresh Codex sessions use an independent process (`--no-daemon`) and workspace-write
with the mailbox directory added. This avoids accidentally inheriting another
agent's shell environment from a shared daemon. It does **not** change the logical
identity when switching runtimes. Existing permission choices can be recorded in
`runtime_args`; ordinary provider permission prompts still apply.

The Codex bootstrap asks the agent to bind its exact session:

```sh
agentmsg bind ships --runtime codex       # inside that agent: reads CODEX_THREAD_ID
```

From another shell, supply `--session UUID`. Fresh Claude sessions receive an
explicit ID at launch. Prefer exact IDs over `--last`/`--continue`, especially when
several agents share one project directory.

To adopt an existing Claude session, `claude agents --json` identifies its name,
UUID, PID and current status in supported CLI versions. Match the actual session,
not just its directory. Register its role if necessary, then bind its UUID:

```sh
agentmsg bind engine --runtime claude --session UUID
# Exit the existing unmanaged copy at a safe stopping point, then:
agentmsg launch engine
```

`bind` does not stop or move a running conversation. Do not run two copies of it.
An unmanaged idle Claude terminal cannot be woken just by notification; its Stop
hook only checks mail when a turn ends. Put it under the managed launcher for
continuous delivery. Advanced hook/wake setup is in the [reference](../../docs/agent-messaging.md).

## Switch Claude ↔ Codex without losing the agent

`agentmsg switch NAME --to RUNTIME` requests a context spool and waits up to 180
seconds. The source writes `CONTEXT.md` with goals, decisions, changes, tests,
unfinished work, approval boundaries and outstanding threads. It marks the handoff
ready and stops work; delivery pauses while the launcher replaces the CLI in the
same managed terminal. The destination reads that context and keeps the mailbox.

This is a context handoff, not a fabricated native conversation import. Source
session files remain intact. Each switch carries forward the latest work, including
work completed in the other runtime.

If the wait expires, use the printed `--handoff PATH` continuation command. Do not
create repeated spool requests. To prepare separately:

```sh
agentmsg spool ships
agentmsg switch ships --to codex --handoff /path/printed/by/spool
```

If the source has already exhausted its quota:

```sh
agentmsg spool ships --snapshot
# Exit the source, then use the printed snapshot directory:
agentmsg switch ships --to codex --handoff /path/to/snapshot
```

The snapshot saves available transcript records, mail and Git context without a
model turn. It is explicitly labelled as an automatic recovery, not an agent-written
summary. If the session is unbound, bind its exact ID or supply `spool --source
/path/to/session.jsonl`; otherwise the manifest can report zero exported records.
Snapshots do not capture process memory or unsaved editor buffers.

`agentmsg handoff-cancel NAME` resumes delivery and invalidates a ready handoff if
you keep working in the source. Used handoffs cannot be reused. Handoff files and
switch configuration backups live under `~/.agents/handoffs/` and `~/.agents/switches/`.

## Send work and reply

```sh
agentmsg inbox --as ships --peek          # inspect without consuming mail
agentmsg inbox --as ships                 # claim unread mail
agentmsg open --as ships                  # owed work and pending replies
agentmsg send engine task --as ships --summary 'Check the power budget' --ref ship-key --see /path/to/details
agentmsg send ships done --as engine --reply-to MESSAGE_ID --summary 'Checked; results in /path/to/report'
```

Summaries have a 400-character limit. Use `--see` for detail and `--reply-to` to
close the original thread. Reply `blocked` when unable to complete a task. Do not
answer routine acknowledgements, notes or completion messages. Check mail at the
start and end of your work; handle incoming tasks after the current task.

Identity is explicit `--as`, then `AGENTMSG_NAME`, then the configured working
directory match. At a neutral directory such as your home, “who are you?” is
expected: select the intended identity rather than impersonating an unrelated agent.

## Verify delivery and troubleshoot

```sh
agentmsg probe ships engine --wait 60
agentmsg probe --check probe-ID --wait 60
systemctl --user status agentmsg --no-pager
journalctl --user -u agentmsg -n 30 --no-pager
```

A fresh `PASS` requires both agents to send a nonce-tagged ping and both peers to
acknowledge the correct message thread. The probe helpers are executed by the
agents, not by the operator on their behalf. A timeout is `PENDING`, not proof of
failure. Recheck that probe instead of sending more probes; it expires after one
hour. Live probes use model turns. A historical pass does not establish current
connectivity after a restart.

| Observation | Check or action |
|---|---|
| Notification delivery, no pong | Launch the correct live conversation under `agentmsg`; verify the configured session isn't stale. |
| Terminal delivery, no pong yet | Inspect its terminal for active work, permission prompts or quota exhaustion. Mail settles for 20 seconds before delivery. |
| Reply went to an old name | Check aliases and confirm old mailbox files were consolidated. |
| Source is open elsewhere | Identify the exact conversation and stop the old copy safely before launching another. |
| Handoff waiting | Inspect the source terminal and `manifest.json`; resume the printed continuation command when ready. |
| Usage unknown | Bind the exact session. Usage is current-session telemetry, not account billing; quota observations can be stale. |

Use role names independent of provider: `ships`, not a second permanent identity
for each runtime. `~/.agents/aliases.json` can preserve old names:

```json
{"ships-claude": "ships", "engine-claude": "engine"}
```

Aliases route future traffic. Consolidating existing identities also requires a
configuration/mail backup and merging their mailbox files without losing read
state or changing message IDs. A renamed session may retain its server socket via
`tmux_socket` in the agent's config. See the [reference](../../docs/agent-messaging.md#canonical-names-and-compatibility-aliases)
for this maintenance operation; there is no automatic rename/merge command.

## Install the operating skill

The portable skill is [skills/agentmsg/SKILL.md](skills/agentmsg/SKILL.md). From the
repository root, install directory links for either or both clients:

```sh
agentmsg_repo="$PWD"
mkdir -p "$HOME/.codex/skills" "$HOME/.claude/skills"
ln -s "$agentmsg_repo/tools/agents/skills/agentmsg" "$HOME/.codex/skills/agentmsg"
ln -s "$agentmsg_repo/tools/agents/skills/agentmsg" "$HOME/.claude/skills/agentmsg"
```

Inspect existing paths before replacing them. If using a custom `CODEX_HOME`, use
its `skills` directory. Start a new client session if the current one does not
refresh skill discovery.

## Validate changes

From the repository root:

```sh
python -m unittest discover -s tools/agents -p 'test_*.py' -v
```

Tests use isolated mailboxes. Live verification has also covered a Claude ↔ Codex
context round trip, preserving a terminal across switches, and two-way pings
between the actual `ships` and `engine` agents. Keep private mail, session exports,
local configuration and account usage out of repository commits.
