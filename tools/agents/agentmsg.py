#!/usr/bin/env python3
"""agentmsg: a tiny mailbox for agents (Claude Code, Codex, anything with a shell). Files, not sockets; no dependencies.

    agentmsg send <to> <kind> --summary "..." [--ref KEY] [--see PATH]... [--reply-to ID] [--as ME]
    agentmsg inbox [--as ME] [--peek] [--quiet]     unread summaries, one line each; marks them read (unless --peek)
    agentmsg show <id>                               one message in full
    agentmsg wait [--as ME] [--timeout S]            block until mail arrives, then print it (inotify; slow poll elsewhere)
    agentmsg serve                                   watch every inbox; wake or notify the agent mail is for
    agentmsg hook-claude                             a Claude Code Stop hook: unread mail keeps the session going
    agentmsg who                                     the agents configured, their inboxes and wake commands
    agentmsg term <agent> [--here] [-- CMD ...]      run the agent in a terminal you can watch and type into (tmux
                                                     session agent-<name>); `serve` types its mail into it and notifies you

kind: task | ack | done | blocked | note. A summary is at most 300 characters: put detail in files, commits and keys and
point at them with --ref and --see; the receiver opens them only if it needs to. Reply on a thread with --reply-to.

Who am I: --as, else $AGENTMSG_NAME, else the agent in ~/.agents/agents.json whose "cwd" holds the current directory.
The mailbox: $AGENTMSG_HOME, else ~/.agents. Docs: docs/agent-messaging.md.
"""
import datetime, glob, json, os, secrets, shutil, subprocess, sys, time

HOME = os.environ.get("AGENTMSG_HOME") or os.path.expanduser("~/.agents")
MAIL, LOG, RUN = os.path.join(HOME, "mail"), os.path.join(HOME, "log"), os.path.join(HOME, "run")
CONF = os.path.join(HOME, "agents.json")
KINDS = ("task", "ack", "done", "blocked", "note")
MAX_SUMMARY = 300


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
    s = opt.get("summary") if isinstance(opt.get("summary"), str) else die("--summary '...' (at most 300 characters)")
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


def wake_prompt(name, got):
    lines = "\n".join(line(m) for _, m in got)
    return (f"Mail for you ({name}), {len(got)} message(s):\n{lines}\n"
            f"Handle it, then reply with `agentmsg send <to> <ack|done|blocked|note> --summary '...' --reply-to <id> --as {name}` "
            f"(summary at most 300 characters; point at files and commits). The messages are marked read.")


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
    while True:
        agents = conf()
        names = sorted(set(os.listdir(MAIL)) | set(agents)) if os.path.isdir(MAIL) else sorted(agents)
        for name in names:
            got = unread(name)
            if not got:
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
        watch([MAIL], 60, recursive=True)      # (the whole tree: an agent's first mail makes its folder)


def cmd_hook_claude(pos, opt):
    """Claude Code Stop hook: if this session's agent has unread mail, block the stop and hand the summaries in."""
    try:
        data = json.load(sys.stdin)
    except Exception:
        data = {}
    name = me(opt, data.get("cwd"))
    if not name:
        return
    got = unread(name)
    if not got:
        return
    mark_read(name, [f for f, _ in got])
    print(json.dumps({"decision": "block", "reason": wake_prompt(name, got)}))


def tmux_alive(sess):
    return shutil.which("tmux") and subprocess.run(["tmux", "has-session", "-t", sess], capture_output=True).returncode == 0


def inject(name, sess, got):
    """Type the mail into the agent's terminal as one line and press Enter: a Claude or Codex prompt takes it as a message
    (queued if the agent is mid-turn)."""
    text = " | ".join(wake_prompt(name, got).splitlines())
    subprocess.run(["tmux", "send-keys", "-t", sess, "-l", text], check=False)
    time.sleep(0.3)
    subprocess.run(["tmux", "send-keys", "-t", sess, "Enter"], check=False)


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
    if not tmux_alive(sess):
        cmd = c.get("term") or die(f"how is {name} started? give it once: agentmsg term {name} -- codex   (or -- claude)")
        env = ["-e", f"AGENTMSG_NAME={name}", "-e", f"PATH={os.path.expanduser('~/bin')}{os.pathsep}{os.environ.get('PATH', '')}"]
        subprocess.run(["tmux", "new-session", "-d", "-s", sess, "-c", os.path.expanduser(c.get("cwd", "~")), *env, *cmd], check=True)
        print(f"started {name} in tmux session {sess} ({' '.join(cmd)})")
    if opt.get("here") and os.environ.get("TMUX"):
        os.execvp("tmux", ["tmux", "switch-client", "-t", sess])
    if not opt.get("here") or not sys.stdout.isatty():
        term = os.environ.get("TERMINAL") or next((t for t in ("alacritty", "kitty", "foot", "gnome-terminal", "xterm") if shutil.which(t)), None)
        if not term:
            die(f"no terminal found: attach with  tmux attach -t {sess}")
        subprocess.Popen([term, "-e", "tmux", "attach", "-t", sess], start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print(f"opened a {term} window on {sess}; detach with Ctrl-b d, the agent keeps running")
    else:
        os.execvp("tmux", ["tmux", "attach", "-t", sess])


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
     "hook-claude": cmd_hook_claude, "who": cmd_who}.get(argv[1], lambda p, o: die(f"no command {argv[1]}"))(pos, opt)


if __name__ == "__main__":
    main(sys.argv)
