import argparse
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).parent
spec = importlib.util.spec_from_file_location('agentmsg_test', HERE / 'agentmsg.py')
api = importlib.util.module_from_spec(spec)
spec.loader.exec_module(api)
import agentnet as net


class NetworkTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.env = patch.dict(os.environ, {'AGENTMSG_HOME': str(self.root), 'CODEX_HOME': str(self.root / 'codex'), 'CLAUDE_CONFIG_DIR': str(self.root / 'claude')})
        self.env.start()
        self.fields = patch.multiple(api, HOME=str(self.root), MAIL=str(self.root / 'mail'), CONF=str(self.root / 'agents.json'), RUN=str(self.root / 'run'))
        self.fields.start()
        net.API = api
        net.save(api.CONF, {'a': {'cwd': str(self.root), 'runtime': 'codex', 'term': ['codex']}, 'b': {'cwd': str(self.root), 'runtime': 'claude', 'term': ['claude']}})

    def tearDown(self):
        self.fields.stop(); self.env.stop(); self.tmp.cleanup()

    def call(self, argv):
        with contextlib.redirect_stdout(io.StringIO()) as out, contextlib.redirect_stderr(io.StringIO()) as err:
            code = net.main(api, argv)
        return code, out.getvalue(), err.getvalue()

    def probe(self):
        code, out, err = self.call(['probe', 'a', 'b'])
        self.assertEqual(code, 1, err)
        p = next((self.root / 'probes').glob('*.json'))
        return net.read(p)

    def test_probe_requires_both_agent_pings_and_correlated_acks(self):
        p = self.probe()
        pid = p['id']
        self.assertEqual(net.probe_result(p, net.messages())['state'], 'PENDING')
        for src, dst in [('a', 'b'), ('b', 'a')]:
            code, _, err = self.call(['probe-send', pid, '--as', src])
            self.assertEqual(code, 0, err)
            ping = next(m for m in net.messages() if m.get('ref') == pid and m['from'] == src and m['to'] == dst and m['kind'] == 'task')
            # Wrong actor cannot acknowledge its own ping.
            self.assertEqual(self.call(['probe-ack', pid, ping['id'], '--as', src])[0], 2)
            self.assertEqual(self.call(['probe-ack', pid, ping['id'], '--as', dst])[0], 0)
            # Replays do not create duplicate messages.
            count = len(net.messages())
            self.call(['probe-ack', pid, ping['id'], '--as', dst])
            self.call(['probe-send', pid, '--as', src])
            self.assertEqual(len(net.messages()), count)
            if src == 'a':
                self.assertEqual(net.probe_result(p, net.messages())['state'], 'PENDING')
        self.assertEqual(net.probe_result(p, net.messages())['state'], 'PASS')
        self.assertEqual(self.call(['probe', '--check', pid])[0], 0)

    def test_expired_probe_does_not_send(self):
        p = self.probe(); p['expires'] = 0
        net.save(self.root / 'probes' / (p['id'] + '.json'), p)
        self.assertEqual(self.call(['probe-send', p['id'], '--as', 'a'])[0], 2)

    def test_legacy_codex_ignores_stale_claude_hook_id(self):
        one, two = '11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'
        self.assertEqual(net.identity({'term': ['codex', 'resume', one], 'session': two}), ('codex', one))
        self.assertEqual(net.identity({'term': ['codex', 'resume', '--last'], 'session': two}), ('codex', None))

    def test_usage_deduplicates_claude_content_blocks_and_codex_totals(self):
        f = self.root / 'usage.jsonl'
        rows = [{'type': 'assistant', 'message': {'id': 'same', 'usage': {'input_tokens': 10, 'output_tokens': 3}}}] * 2
        rows.append({'type': 'assistant', 'message': {'id': 'same', 'usage': {'input_tokens': 10, 'output_tokens': 7}}})
        f.write_text('\n'.join(map(json.dumps, rows)))
        self.assertEqual(net.usage(f, 'claude')['tokens']['input_tokens'], 10)
        self.assertEqual(net.usage(f, 'claude')['tokens']['output_tokens'], 7)
        rows = [{'type': 'event_msg', 'payload': {'type': 'token_count', 'info': {'total_token_usage': {'input_tokens': n}}}} for n in (20, 40)]
        f.write_text('\n'.join(map(json.dumps, rows)))
        self.assertEqual(net.usage(f, 'codex')['tokens']['input_tokens'], 40)

    def test_status_deduplicates_sent_and_received_copies(self):
        mid = net.send('b', 'task', 'work', 'a')
        with patch.object(api, 'tmux_alive', return_value=False), patch.object(api, 'running', return_value=False):
            code, out, _ = self.call(['status', '--json'])
        result = json.loads(out)
        self.assertEqual(result['unique_messages'], 1)
        self.assertEqual(result['agents'][0]['waiting'], 1)
        self.assertEqual(result['agents'][1]['owe'], 1)
        net.send('a', 'done', 'finished', 'b', **{'reply-to': mid})
        with patch.object(api, 'tmux_alive', return_value=False), patch.object(api, 'running', return_value=False):
            _, out, _ = self.call(['status', '--json'])
        self.assertEqual(json.loads(out)['agents'][0]['waiting'], 0)

    def test_snapshot_keeps_original_and_can_switch_without_launch(self):
        source = self.root / 'source.jsonl'
        source.write_text(json.dumps({'type': 'response_item', 'payload': {'type': 'message', 'role': 'user', 'content': [{'type': 'input_text', 'text': 'continue this task'}]}}) + '\n')
        original = source.read_bytes()
        code, out, err = self.call(['spool', 'a', '--snapshot', '--source', str(source)])
        self.assertEqual(code, 0, err)
        folder = Path(out.strip())
        self.assertIn('continue this task', (folder / 'transcript.jsonl').read_text())
        with patch.object(api, 'tmux_alive', return_value=False), patch('shutil.which', return_value='/bin/true'):
            code, out, err = self.call(['switch', 'a', '--to', 'claude', '--handoff', str(folder), '--no-start'])
        self.assertEqual(code, 0, err)
        self.assertEqual(source.read_bytes(), original)
        c = api.conf()['a']
        self.assertEqual(c['runtime'], 'claude')
        self.assertIn(str(folder), c['term'][-1])
        self.assertTrue(net.read(folder / 'manifest.json')['consumed_at'])
        self.assertEqual(self.call(['switch', 'a', '--to', 'codex', '--handoff', str(folder), '--no-start'])[0], 2)

    def test_snapshot_will_not_kill_a_live_agent(self):
        _, out, _ = self.call(['spool', 'a', '--snapshot'])
        with patch.object(api, 'tmux_alive', return_value=True), patch('shutil.which', return_value='/bin/true'), patch('subprocess.run') as run:
            code, _, _ = self.call(['switch', 'a', '--to', 'claude', '--handoff', out.strip(), '--no-start'])
        self.assertEqual(code, 2)
        run.assert_not_called()
        self.assertEqual(api.conf()['a']['runtime'], 'codex')

    def test_agent_handoff_requires_written_context_and_pauses_delivery(self):
        _, out, _ = self.call(['spool', 'a'])
        folder = Path(out.strip())
        with patch.dict(os.environ, {'AGENTMSG_NAME': 'a'}):
            self.assertEqual(self.call(['handoff-ready', str(folder)])[0], 2)
            (folder / 'CONTEXT.md').write_text('Current task: implement the new network. Decisions: preserve agent identity. Files changed: agentnet.py. Tests: pending. Next: run tests. ')
            self.assertEqual(self.call(['handoff-ready', str(folder)])[0], 0)
        self.assertEqual(api.conf()['a']['handoff_pause'], str(folder))
        self.assertTrue(net.read(folder / 'manifest.json')['ready'])
        self.call(['handoff-cancel', 'a'])
        self.assertNotIn('handoff_pause', api.conf()['a'])

    def test_unready_handoff_does_not_change_agent(self):
        _, out, _ = self.call(['spool', 'a'])
        self.assertEqual(self.call(['switch', 'a', '--to', 'claude', '--handoff', out.strip(), '--wait', '0'])[0], 1)
        self.assertEqual(api.conf()['a']['runtime'], 'codex')

    def test_claim_prevents_duplicate_delivery_and_restore_requeues(self):
        net.send('b', 'task', 'delivery test', 'a')
        snapshot = api.unread('b')
        claimed = api.claim('b', snapshot)
        self.assertEqual(len(claimed), 1)
        self.assertEqual(api.claim('b', snapshot), [])
        api.restore('b', claimed)
        self.assertEqual(len(api.unread('b')), 1)

    def test_new_codex_launch_is_independent_and_identity_is_explicit(self):
        options = net.runtime_options({}, 'codex')
        self.assertIn('--no-daemon', options)
        self.assertIn('workspace-write', options)
        prompt = net.bootstrap('a')
        self.assertIn('AGENTMSG_NAME=a', prompt)
        self.assertIn('AGENTMSG_HOME=' + str(self.root), prompt)

    def test_cancelled_handoff_cannot_be_marked_ready_again(self):
        _, out, _ = self.call(['spool', 'a'])
        folder = Path(out.strip())
        (folder / 'CONTEXT.md').write_text('Current task and user constraints. ' * 10)
        with patch.dict(os.environ, {'AGENTMSG_NAME': 'a'}):
            self.assertEqual(self.call(['handoff-ready', str(folder)])[0], 0)
            self.call(['handoff-cancel', 'a'])
            self.assertEqual(self.call(['handoff-ready', str(folder)])[0], 2)
        self.assertEqual(self.call(['switch', 'a', '--to', 'claude', '--handoff', str(folder), '--wait', '0'])[0], 2)

    def test_switch_respawns_same_terminal_and_retires_stale_requests(self):
        _, stale, _ = self.call(['spool', 'a'])
        _, current, _ = self.call(['spool', 'a'])
        folder = Path(current.strip())
        (folder / 'CONTEXT.md').write_text('Current task and user constraints. ' * 10)
        with patch.dict(os.environ, {'AGENTMSG_NAME': 'a'}):
            self.call(['handoff-ready', str(folder)])
        with patch.object(api, 'tmux_alive', return_value=True), patch.object(api, 'tx', return_value=['tmux']), patch.object(api, 'start', return_value=True), patch('shutil.which', return_value='/bin/true'), patch('subprocess.run') as run:
            code, _, err = self.call(['switch', 'a', '--to', 'claude', '--handoff', str(folder), '--no-windows'])
        self.assertEqual(code, 0, err)
        self.assertIn('respawn-pane', run.call_args.args[0])
        self.assertNotIn('kill-session', run.call_args.args[0])
        self.assertTrue(net.read(Path(stale.strip()) / 'manifest.json')['cancelled_at'])
        stale_id = net.read(Path(stale.strip()) / 'manifest.json')['request']
        self.assertNotIn(stale_id, [m['id'] for _, m in api.unread('a')])

    def test_register_bind_and_unknown_identity(self):
        self.assertEqual(self.call(['register', '../bad', '--cwd', str(self.root), '--runtime', 'codex'])[0], 2)
        self.assertEqual(self.call(['register', 'new', '--cwd', str(self.root), '--runtime', 'codex'])[0], 0)
        sid = '11111111-1111-4111-8111-111111111111'
        with patch.dict(os.environ, {'CODEX_THREAD_ID': sid}):
            self.assertEqual(self.call(['bind', 'new', '--runtime', 'codex'])[0], 0)
        self.assertEqual(api.conf()['new']['session'], sid)


if __name__ == '__main__':
    unittest.main()
