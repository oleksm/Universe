# Agent messaging: agentmsg

A mailbox any agent with a shell can use: Claude Code sessions, Codex, scripts. Files, not sockets, so it works across
tools, survives restarts and power cuts, and costs no tokens while an agent waits. `tools/agents/agentmsg.py`, linked as
`~/bin/agentmsg`; the mailbox is `~/.agents` (or `$AGENTMSG_HOME`).

## Commands

    agentmsg send <to> <kind> --summary "..." [--ref KEY] [--see PATH]... [--reply-to ID] [--as ME]
    agentmsg inbox [--as ME] [--peek] [--quiet]     unread summaries, one line each; marks them read
    agentmsg show <id>                               one message in full
    agentmsg wait [--as ME] [--timeout S]            block until mail arrives (inotify), then print it
    agentmsg serve                                   watch every inbox: wake or notify the agent mail is for
    agentmsg who                                     agents, their unread counts, auto or notify

`kind` is one of task, ack, done, blocked, note. A summary is at most 300 characters: detail lives in files, commits
and registry keys, pointed at with `--ref` and `--see`, and the receiver opens them only if it needs them. A hand-off is
a `task`, answered by an `ack`, then `done` (with a commit or a path) or `blocked` (one line why), each `--reply-to` the
task's id.

## Who is who: `~/.agents/agents.json`

    {"registry": {"cwd": "~/git/universe-fso"},
     "blender":  {"cwd": "~/git/blender", "wake": ["codex", "exec", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox", "{prompt}"]}}

An agent is known by `--as`, else `$AGENTMSG_NAME`, else the entry whose `cwd` holds the current directory.

## Being told

- **A Claude Code session** has a Stop hook (`~/.claude/settings.json`: `agentmsg hook-claude`): when the session is
  about to end its turn and its agent has unread mail, the hook keeps it going with the summaries.
- **An idle agent**: `agentmsg serve` (run it once in a terminal, or as a user service) watches the inboxes. For an
  agent with a `wake` command it starts that command with the mail as the prompt (`{prompt}`), one run at a time,
  logging to `~/.agents/log/<agent>.log`; the agent reads, works and replies with `agentmsg send`. For an agent without
  one it sends a desktop notification. Wake is opt-in per agent: a Codex agent wakes with `codex exec … "{prompt}"`
  (or `codex exec resume <session> "{prompt}"`); a Claude agent with `claude -p --resume <session> "{prompt}"` plus the
  permission flags it needs.
- **An agent waiting on a reply** runs `agentmsg wait` in the background: it returns when the mail arrives.

## Tested

2026-10-10: a task sent to a Codex agent with a wake command was answered (`done … pong`) in 7 seconds through
`serve`; the Stop hook, `wait`, `inbox --quiet` and `who` checked by hand.
