"""Tests for early_ack_analysis.py against the sample captures in samples/.

Run:  python3 -m unittest .scratch/internet-play-speed/analysis/test_early_ack_analysis.py
(or from this directory: python3 -m unittest test_early_ack_analysis)
"""
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import early_ack_analysis as ea  # noqa: E402

PEER = 'aaaa0001'


def sample(name):
    return ea.load(os.path.join(HERE, 'samples', name))


class HopTime(unittest.TestCase):
    def test_hops_run_from_written_to_the_friends_next_in(self):
        h = ea.hops(sample('early-acks-ok.jsonl'), PEER)
        # 2100 -> 61100 and 72050 -> 140000
        self.assertEqual((h['hops'], h['hop_us']), (2, 59000 + 67950))

    def test_network_time_is_hops_plus_dll_polls(self):
        h = ea.hops(sample('early-acks-ok.jsonl'), PEER)
        # Swallowed acks are never drained; the friend's data waited 1000, 500 and 1500 us.
        self.assertEqual(h['dll_poll_us'], 1000 + 500 + 1500)
        self.assertEqual(h['network_us'], h['hop_us'] + h['dll_poll_us'])


class DeliveryCheck(unittest.TestCase):
    def test_a_clean_capture_passes_all_three_checks(self):
        results = ea.delivery_check(sample('early-acks-ok.jsonl'), PEER)
        self.assertEqual(len(results), 3)
        for name, (ok, detail) in results.items():
            self.assertTrue(ok, f'{name}: {detail}')

    def test_a_broken_capture_fails_each_check_and_says_why(self):
        r = ea.delivery_check(sample('early-acks-broken.jsonl'), PEER)
        ok, detail = r['one real ack per early ack']
        self.assertFalse(ok)
        self.assertIn('no real ack for seqs [2]', detail)
        ok, detail = r["no gaps in the friend's seqs"]
        self.assertFalse(ok)
        self.assertIn('missing [3, 4]', detail)
        ok, detail = r['every message drained once']
        self.assertFalse(ok)
        self.assertIn('never drained: ids [4, 6]', detail)
        self.assertIn('more than once: ids [5]', detail)

    def test_the_command_exits_non_zero_on_a_failing_capture(self):
        import contextlib
        import io
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(ea.main(['x', os.path.join(HERE, 'samples', 'early-acks-ok.jsonl')]), 0)
            self.assertEqual(ea.main(['x', os.path.join(HERE, 'samples', 'early-acks-broken.jsonl')]), 1)


if __name__ == '__main__':
    unittest.main()
