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
    agentmsg open [--as ME]                          what I owe and what I wait on, a few lines
    agentmsg who                                     agents, their unread counts, auto or notify

`kind` is one of task, ack, done, blocked, note. A summary is at most 300 characters: detail lives in files, commits
and registry keys, pointed at with `--ref` and `--see`, and the receiver opens them only if it needs them. A hand-off is
a `task`, answered by `done` (with a commit or a path) or `blocked` (one line why), `--reply-to` the task's id; an
`ack` in between is optional.

## Spending few tokens

The message is cheap (about 60 tokens); the wake is not: a turn re-reads the agent's whole conversation (engine's was
622k tokens a call on 2026-10-10). So the rules cut wakes, not words:

- **Answer only a task, a blocked or a question** (a summary with `?`). The delivered text names the ids that want an
  answer and says the rest needs none. Never answer an ack, a note or a done.
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
detach with Ctrl-b d and it keeps running. While its session is up, `serve` types each piece of mail into it as one
line and presses Enter (Codex and Claude take it as your next message, queued if mid-turn), and sends a desktop
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
