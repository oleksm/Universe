#!/usr/bin/env python3
"""agentmsg: a tiny mailbox for agents (Claude Code, Codex, anything with a shell). Files, not sockets; no dependencies.

    agentmsg send <to> <kind> --summary "..." [--ref KEY] [--see PATH]... [--reply-to ID] [--as ME]
    agentmsg inbox [--as ME] [--peek] [--quiet]     unread summaries, one line each; marks them read (unless --peek)
    agentmsg show <id>                               one message in full
    agentmsg wait [--as ME] [--timeout S]            block until mail arrives, then print it (inotify; slow poll elsewhere)
    agentmsg serve                                   watch every inbox; wake or notify the agent mail is for
    agentmsg hook-claude                             a Claude Code Stop hook: unread mail keeps the session going
    agentmsg up [AGENT...] [--no-windows]            start every agent (or those named) that is down, a window on each
                                                     nobody watches, the mail watcher; safe to run again any time
    agentmsg open [--as ME]                          what I owe (tasks, blocks to me unanswered) and wait on (my tasks)
    agentmsg who                                     the agents configured, their inboxes and wake commands
    agentmsg term <agent> [--here] [-- CMD ...]      run the agent in a terminal you can watch and type into (tmux
                                                     session agent-<name>); `serve` types its mail into it and notifies you

kind: task | ack | done | blocked | note. Answer only a task, a blocked or a question; acks never wake anyone (they come
with the next mail), and an inbox is delivered once it has been quiet for $AGENTMSG_SETTLE s (20), as one wake. A summary is at most 400 characters: put detail in files, commits and keys and
point at them with --ref and --see; the receiver opens them only if it needs to. Reply on a thread with --reply-to.

Who am I: --as, else $AGENTMSG_NAME, else the agent in ~/.agents/agents.json whose "cwd" holds the current directory.
The mailbox: $AGENTMSG_HOME, else ~/.agents. Docs: docs/agent-messaging.md.
"""
import datetime, glob, json, os, re, secrets, shutil, subprocess, sys, time

HOME = os.environ.get("AGENTMSG_HOME") or os.path.expanduser("~/.agents")
MAIL, LOG, RUN = os.path.join(HOME, "mail"), os.path.join(HOME, "log"), os.path.join(HOME, "run")
CONF = os.path.join(HOME, "agents.json")
KINDS = ("task", "ack", "done", "blocked", "note")
MAX_SUMMARY = 400
SETTLE = float(os.environ.get("AGENTMSG_SETTLE", 20))     # s of quiet in an inbox before its mail is delivered, as one wake


def die(msg, code=2):
    print(f"agentmsg: {msg}", file=sys.stderr)
    sys.exit(code)


def conf():
    try:
        return json.load(open(CONF))
    except FileNotFoundError:
        return {}


def me(opt, cwd=None):
    if opt.get("as"):
        return opt["as"]
    if os.environ.get("AGENTMSG_NAME"):
        return os.environ["AGENTMSG_NAME"]
    cwd = os.path.realpath(cwd or os.getcwd())
    best = None
    for name, c in conf().items():
        d = os.path.realpath(os.path.expanduser(c.get("cwd", ""))) if c.get("cwd") else None
        if d and (cwd == d or cwd.startswith(d + os.sep)) and (best is None or len(d) > len(best[1])):
            best = (name, d)
    return best[0] if best else None


def box(name, part="new"):
    p = os.path.join(MAIL, name, part)
    os.makedirs(p, exist_ok=True)
    return p


def write_atomic(path, data):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(data, f, ensure_ascii=False)
    os.replace(tmp, path)


def line(m):
    see = f" see {', '.join(m['see'])}" if m.get("see") else ""
    ref = f" [{m['ref']}]" if m.get("ref") else ""
    thr = f" re {m['reply_to']}" if m.get("reply_to") else ""
    return f"{m['id']} {m['kind']} from {m['from']}{ref}{thr}: {m['summary']}{see}"


def unread(name):
    out = []
    for f in sorted(glob.glob(os.path.join(box(name), "*.json"))):
        try:
            out.append((f, json.load(open(f))))
        except (json.JSONDecodeError, OSError):
            pass
    return out


def mark_read(name, files):
    for f in files:
        try:
            os.replace(f, os.path.join(box(name, "read"), os.path.basename(f)))
        except FileNotFoundError:
            pass


def parse(argv):
    pos, opt, i = [], {}, 0
    while i < len(argv):
        a = argv[i]
        if a.startswith("--"):
            k, _, v = a[2:].partition("=")
            if not _:
                if i + 1 < len(argv) and not argv[i + 1].startswith("--") and k not in ("peek", "quiet", "window"):
                    v = argv[i + 1]; i += 1
                else:
                    v = True
            if k == "see":
                opt.setdefault("see", []).append(v)
            else:
                opt[k] = v
        else:
            pos.append(a)
        i += 1
    return pos, opt


def cmd_send(pos, opt):
    if len(pos) < 2:
        die("send <to> <kind> --summary '...'")
    to, kind = pos[0], pos[1]
    if kind not in KINDS:
        die(f"kind is one of {', '.join(KINDS)}")
    frm = me(opt) or die("who are you? --as NAME, $AGENTMSG_NAME, or a cwd in ~/.agents/agents.json")
    s = opt.get("summary") if isinstance(opt.get("summary"), str) else die(f"--summary '...' (at most {MAX_SUMMARY} characters)")
    if len(s) > MAX_SUMMARY:
        die(f"the summary is {len(s)} characters; at most {MAX_SUMMARY}: put the rest in a file and --see it")
    now = datetime.datetime.now()
    mid = f"{now:%Y%m%dT%H%M%S}-{frm}-{secrets.token_hex(2)}"
    m = {"id": mid, "from": frm, "to": to, "kind": kind, "summary": s, "at": now.isoformat(timespec="seconds")}
    for k in ("ref", "reply_to"):
        if isinstance(opt.get(k.replace("_", "-")), str):
            m[k] = opt[k.replace("_", "-")]
    if opt.get("see"):
        m["see"] = opt["see"]
    write_atomic(os.path.join(box(to), mid + ".json"), m)
    shutil.copyfile(os.path.join(box(to), mid + ".json"), os.path.join(box(frm, "sent"), mid + ".json"))
    print(mid)


def cmd_inbox(pos, opt):
    name = me(opt) or die("who are you? --as NAME")
    got = unread(name)
    if not got:
        if not opt.get("quiet"):
            print("no mail")
        sys.exit(1 if opt.get("quiet") else 0)
    for _, m in got:
        print(line(m))
    if not opt.get("peek"):
        mark_read(name, [f for f, _ in got])


def cmd_show(pos, opt):
    if not pos:
        die("show <id>")
    hits = glob.glob(os.path.join(MAIL, "*", "*", pos[0] + ".json"))
    if not hits:
        die(f"no message {pos[0]}")
    print(json.dumps(json.load(open(hits[0])), indent=1, ensure_ascii=False))


def watch(dirs, timeout=None, recursive=False):
    """Block until a file appears in one of `dirs` (or the timeout passes): inotifywait where there is one, else a poll."""
    if shutil.which("inotifywait"):
        args = ["inotifywait", "-q", "-e", "moved_to", "-e", "close_write"] + (["-r"] if recursive else []) + (["-t", str(int(timeout))] if timeout else []) + dirs
        subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        end = time.time() + (timeout or 1e12)
        before = {d: set(os.listdir(d)) for d in dirs}
        while time.time() < end and all(set(os.listdir(d)) == before[d] for d in dirs):
            time.sleep(2)


def cmd_wait(pos, opt):
    name = me(opt) or die("who are you? --as NAME")
    timeout = float(opt["timeout"]) if isinstance(opt.get("timeout"), str) else None
    end = time.time() + timeout if timeout else None
    while not unread(name):
        left = end - time.time() if end else None
        if left is not None and left <= 0:
            print("no mail"); sys.exit(1)
        watch([box(name)], left)
    cmd_inbox([], {"as": name})


def owed(m):
    """A message that wants an answer: a task, a blocked, or a question."""
    return m.get("kind") in ("task", "blocked") or "?" in m.get("summary", "")


def wake_prompt(name, got, term=False):
    lines = "\n".join(line(m) for _, m in got)
    ask = [m for _, m in got if owed(m)]
    as_ = "" if term or os.environ.get("AGENTMSG_NAME") == name else f" --as {name}"     # (a term agent knows its name)
    help_ = (f"If you are blocked or need another agent's help, ask by mail: agentmsg send <agent> task|blocked --summary '...'{as_}. "
             "Send results by mail, not only in your window.")
    if not ask:
        return f"Mail for you ({name}), {len(got)} message(s):\n{lines}\nNo reply needed."
    cmds = " ".join(f"agentmsg send {m['from']} done|blocked --reply-to {m['id']}{as_} --summary '<={MAX_SUMMARY} chars, name the commit or file'." for m in ask)
    return (f"Mail for you ({name}), {len(got)} message(s):\n{lines}\n"
            f"Start working on {'it' if len(ask) == 1 else 'them'} (after the current task, if busy). Answer with: {cmds} {help_}"
            + (" No reply to the rest." if len(ask) < len(got) else ""))


def wakes(got):
    """Whether mail is worth a turn: acks never are; they ride along with the next mail or turn that is."""
    return any(m.get("kind") != "ack" for _, m in got)


def running(name):
    p = os.path.join(RUN, name + ".pid")
    try:
        pid = int(open(p).read())
        os.kill(pid, 0)
        return True
    except (FileNotFoundError, ValueError, ProcessLookupError, PermissionError):
        return False


def cmd_serve(pos, opt):
    os.makedirs(LOG, exist_ok=True); os.makedirs(RUN, exist_ok=True)
    print(f"agentmsg serve: watching {MAIL}", flush=True)
    me_file = os.path.realpath(__file__); born = os.path.getmtime(me_file)
    while True:
        if os.path.getmtime(me_file) != born:       # (agentmsg was changed: run the new one, or it keeps the old rules)
            print("agentmsg changed: restarting serve", flush=True)
            os.execv(sys.executable, [sys.executable, me_file, "serve"])
        agents = conf()
        names = sorted(set(os.listdir(MAIL)) | set(agents)) if os.path.isdir(MAIL) else sorted(agents)
        soonest = 60
        for name in names:
            got = unread(name)
            if not got or not wakes(got):
                continue
            quiet = time.time() - max(os.path.getmtime(f) for f, _ in got)
            if quiet < SETTLE:              # (more may be coming: one wake for the lot)
                soonest = min(soonest, SETTLE - quiet + 0.5)
                continue
            c = agents.get(name, {})
            if tmux_alive(f"agent-{name}"):
                mark_read(name, [f for f, _ in got])
                inject(name, f"agent-{name}", got)
                if shutil.which("notify-send"):
                    subprocess.run(["notify-send", f"Mail for {name} (typed into its terminal)", line(got[-1][1])[:200]], check=False)
                print(f"{datetime.datetime.now():%T} typed {len(got)} message(s) into agent-{name}", flush=True)
            elif c.get("wake") and not running(name):
                prompt = wake_prompt(name, got)
                cmd = [a.replace("{prompt}", prompt).replace("{name}", name) for a in c["wake"]]
                mark_read(name, [f for f, _ in got])
                log = open(os.path.join(LOG, name + ".log"), "a")
                log.write(f"\n=== {datetime.datetime.now():%F %T} wake {name}: {len(got)} message(s)\n"); log.flush()
                proc = subprocess.Popen(cmd, cwd=os.path.expanduser(c.get("cwd", "~")), stdout=log, stderr=subprocess.STDOUT, env={**os.environ, "AGENTMSG_NAME": name, "PATH": os.path.dirname(os.path.abspath(sys.argv[0])) + os.pathsep + os.path.expanduser("~/bin") + os.pathsep + os.environ.get("PATH", "")})
                open(os.path.join(RUN, name + ".pid"), "w").write(str(proc.pid))
                print(f"{datetime.datetime.now():%T} woke {name} (pid {proc.pid}) for {len(got)} message(s)", flush=True)
            elif not c.get("wake"):
                key = os.path.join(RUN, name + ".notified")
                newest = got[-1][1]["id"]
                if not os.path.exists(key) or open(key).read() != newest:
                    if shutil.which("notify-send"):
                        subprocess.run(["notify-send", f"Mail for {name}", line(got[-1][1])[:200]], check=False)
                    open(key, "w").write(newest)
                    print(f"{datetime.datetime.now():%T} {name}: {len(got)} unread (notified)", flush=True)
        os.makedirs(MAIL, exist_ok=True)
        watch([MAIL], soonest, recursive=True)      # (the whole tree: an agent's first mail makes its folder)


def cmd_hook_claude(pos, opt):
    """Claude Code Stop hook: if this session's agent has unread mail, block the stop and hand the summaries in."""
    try:
        data = json.load(sys.stdin)
    except Exception:
        data = {}
    sid = data.get("session_id")
    owner = next((n for n, c in conf().items() if sid and c.get("session") == sid), None)
    name = opt.get("as") or os.environ.get("AGENTMSG_NAME") or owner or me(opt, data.get("cwd"))
    if not name or (not owner and sid and conf().get(name, {}).get("session") not in (None, sid)):
        return          # (a Claude session of another agent in a shared folder: the folder alone does not make it this one)
    got = unread(name)
    if not got or not wakes(got):
        return
    mark_read(name, [f for f, _ in got])
    print(json.dumps({"decision": "block", "reason": wake_prompt(name, got)}))


def tx(sess):
    """tmux for one agent's session. Each agent has its own tmux server (socket agent-<name>), so one going down takes no
    other agent with it; a session started before that lives on the default server and is still found there. The server
    starts in a systemd scope of its own: started from inside a terminal it would belong to that terminal's scope, and
    closing that window would take every agent down with it (2026-10-10)."""
    for sock in (["-L", sess], []):
        if subprocess.run(["tmux", *sock, "has-session", "-t", sess], capture_output=True).returncode == 0:
            return ["tmux", *sock]
    return ["tmux", "-L", sess]


def tmux_alive(sess):
    return shutil.which("tmux") and subprocess.run([*tx(sess), "has-session", "-t", sess], capture_output=True).returncode == 0


def unsent(sess, mark):
    """Whether the agent's input box still holds the mail: the lowest prompt line (Codex `›`, Claude `❯`) shows its start."""
    pane = subprocess.run([*tx(sess), "capture-pane", "-p", "-t", sess], capture_output=True, text=True).stdout.splitlines()
    prompts = [ln for ln in pane if ln.lstrip().startswith(("›", "❯"))]
    return bool(prompts) and mark in prompts[-1]


def inject(name, sess, got):
    """Hand the mail to the agent's terminal as one pasted line, then press Enter: a Claude or Codex prompt takes it as a
    message (queued if the agent is mid-turn). Pasted, not typed: a TUI reads fast typing as a paste burst and can take
    the Enter as a new line. If the line is still sitting in the input box, Enter again (up to three times)."""
    text = " | ".join(wake_prompt(name, got, term=True).splitlines())
    buf = f"agentmsg-{name}"
    subprocess.run([*tx(sess), "set-buffer", "-b", buf, text], check=False)
    subprocess.run([*tx(sess), "paste-buffer", "-p", "-d", "-b", buf, "-t", sess], check=False)    # (-p: bracketed paste)
    for _ in range(3):
        time.sleep(0.8)
        subprocess.run([*tx(sess), "send-keys", "-t", sess, "Enter"], check=False)
        time.sleep(0.7)
        if not unsent(sess, text[:24]):
            return True
    print(f"{datetime.datetime.now():%T} {name}: the mail is still in its input box after three Enters", flush=True)
    return False


def start(name, c):
    """Start an agent's tmux session (its own server, in a systemd scope of its own) unless it is up; True if it is up."""
    sess = f"agent-{name}"
    if tmux_alive(sess):
        return True
    cmd = c.get("term")
    if not cmd:
        return False
    sid = re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", " ".join(cmd))
    if sid:       # (the same conversation open in another terminal: two copies would fight over it)
        other = subprocess.run(["pgrep", "-f", f"(claude|codex).*{sid.group(0)}"], capture_output=True, text=True).stdout.split()
        if other:
            print(f"{name}: its conversation is open elsewhere (pid {', '.join(other)}): close it there, then run this again")
            return False
    env = ["-e", f"AGENTMSG_NAME={name}", "-e", f"PATH={os.path.expanduser('~/bin')}{os.pathsep}{os.environ.get('PATH', '')}"]
    own = ["systemd-run", "--user", "--scope", "--quiet", f"--unit={sess}-{secrets.token_hex(3)}"] if shutil.which("systemd-run") else []
    subprocess.run([*own, *tx(sess), "new-session", "-d", "-s", sess, "-c", os.path.expanduser(c.get("cwd", "~")), *env, *cmd], check=True)
    print(f"started {name} in tmux session {sess} ({' '.join(cmd)})")
    if unread(name):        # mail that came while it was not running: typed in once its prompt is up
        subprocess.Popen([sys.executable, os.path.abspath(__file__), "deliver", name], start_new_session=True,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return True


def watched(sess):
    """Whether a terminal shows the session now."""
    return bool(subprocess.run([*tx(sess), "list-clients", "-t", sess], capture_output=True, text=True).stdout.strip())


def window(sess):
    """A terminal window of its own on the session."""
    term = os.environ.get("TERMINAL") or next((t for t in ("alacritty", "kitty", "foot", "gnome-terminal", "xterm") if shutil.which(t)), None)
    if not term:
        die(f"no terminal found: attach with  {' '.join(tx(sess))} attach -t {sess}")
    subprocess.Popen([term, "-e", *tx(sess), "attach", "-t", sess], start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                     env={k: v for k, v in os.environ.items() if k != "TMUX"})     # (run from inside an agent's window: still a window of its own)
    print(f"opened a {term} window on {sess}; detach with Ctrl-b d, the agent keeps running")


def cmd_term(pos, opt, rest=None):
    if not pos:
        die("term <agent> [--here] [-- CMD ...]")
    name = pos[0]
    agents = conf(); c = agents.get(name) or die(f"no agent {name} in {CONF}")
    if rest:
        c["term"] = rest
        agents[name] = c
        os.makedirs(HOME, exist_ok=True); json.dump(agents, open(CONF, "w"), indent=1)
    sess = f"agent-{name}"
    if not c.get("term") and not tmux_alive(sess):
        die(f"how is {name} started? give it once: agentmsg term {name} -- codex   (or -- claude)")
    if not start(name, c):
        sys.exit(1)
    if opt.get("here") and os.environ.get("TMUX"):
        os.execvp("tmux", [*tx(sess), "switch-client", "-t", sess])
    if not opt.get("here") or not sys.stdout.isatty():
        window(sess)
    else:
        os.execvp("tmux", [*tx(sess), "attach", "-t", sess])


def cmd_up(pos, opt):
    """Every agent with a start command: started if down, a window on each nobody is watching (--no-windows: none), and
    the mail watcher running. Safe to run again at any time: what is up is left alone."""
    if shutil.which("systemctl"):
        subprocess.run(["systemctl", "--user", "enable", "--now", "agentmsg.service"], capture_output=True)
    for name, c in conf().items():
        if pos and name not in pos:
            continue
        if not c.get("term"):
            print(f"{name}: no start command (agentmsg term {name} -- <command> once)")
            continue
        if start(name, c) and not opt.get("no-windows") and not watched(f"agent-{name}"):
            window(f"agent-{name}")
            time.sleep(0.5)
    print()
    cmd_who([], {})


def cmd_deliver(pos, opt):
    """Type an agent's waiting mail into its terminal, after giving its prompt time to come up (used by term)."""
    name, sess = pos[0], f"agent-{pos[0]}"
    time.sleep(float(opt.get("delay", 12)))
    got = unread(name)
    if got and wakes(got) and tmux_alive(sess):
        mark_read(name, [f for f, _ in got])
        inject(name, sess, got)


def cmd_open(pos, opt):
    """What is still open for me: tasks and blocks sent to me with no done or blocked from me on their thread (owe), and
    tasks I sent with no done or blocked back (waiting). Reads the mailbox, never prints it."""
    name = me(opt) or die("who are you? --as NAME")
    def load(paths):
        out = []
        for f in paths:
            try:
                out.append(json.load(open(f)))
            except (json.JSONDecodeError, OSError):
                pass
        return out
    mine = load(glob.glob(os.path.join(MAIL, name, "sent", "*.json")))
    to_me = load(glob.glob(os.path.join(MAIL, name, "new", "*.json")) + glob.glob(os.path.join(MAIL, name, "read", "*.json")))
    closed_by_me = {m.get("reply_to") for m in mine if m.get("kind") in ("done", "blocked")}
    closed_to_me = {m.get("reply_to") for m in to_me if m.get("kind") in ("done", "blocked")}
    owe = [m for m in to_me if m.get("kind") in ("task", "blocked") and m["id"] not in closed_by_me]
    wait = [m for m in mine if m.get("kind") == "task" and m["id"] not in closed_to_me]
    for title, ms in (("owe", owe), ("waiting", wait)):
        print(f"{title}: {len(ms)}")
        for m in sorted(ms, key=lambda m: m["id"]):
            print("  " + line(m)[:240])


def cmd_who(pos, opt):
    agents = conf()
    names = sorted(set(os.listdir(MAIL)) | set(agents)) if os.path.isdir(MAIL) else sorted(agents)
    print(f"{'agent':<14} {'unread':>6}  {'wake':<6} cwd")
    for n in names:
        c = agents.get(n, {})
        how = "term" if tmux_alive(f"agent-{n}") else ("auto" if c.get("wake") else "notify")
        print(f"{n:<14} {len(unread(n)):>6}  {how:<6} {c.get('cwd', '')}")


def main(argv):
    if len(argv) < 2 or argv[1] in ("-h", "--help", "help"):
        print(__doc__); return
    rest = None
    if "--" in argv:
        rest = argv[argv.index("--") + 1:]; argv = argv[:argv.index("--")]
    pos, opt = parse(argv[2:])
    if argv[1] == "term":
        return cmd_term(pos, opt, rest)
    {"send": cmd_send, "inbox": cmd_inbox, "show": cmd_show, "wait": cmd_wait, "serve": cmd_serve,
     "hook-claude": cmd_hook_claude, "who": cmd_who, "deliver": cmd_deliver, "open": cmd_open, "up": cmd_up}.get(argv[1], lambda p, o: die(f"no command {argv[1]}"))(pos, opt)


if __name__ == "__main__":
    main(sys.argv)
