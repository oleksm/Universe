"""Deck panel boundaries must share shoes without sharing their panel sockets."""
import collections
import unittest
import mounting


class SharedShoes(unittest.TestCase):
    def test_adjacent_cells_all_sizes(self):
        for length, width in mounting.DECKS:
            with self.subTest(size=(length, width)):
                panel = mounting.deck(length, width)[1]
                shoes = collections.defaultdict(list)
                for shift in (0.0, width):
                    for port in panel['ports']:
                        if not port['id'].startswith('edge-'):
                            continue
                        self.assertEqual(port['interface'], 'interface.insert-m10')
                        x, y, z = port['at']
                        self.assertEqual(z, 0)
                        self.assertLess(abs(x), length / 2)
                        self.assertLess(abs(y), width / 2)
                        if abs(abs(y) - (width / 2 - .04)) < 1e-8:
                            cx, cy = x, (width / 2 if y > 0 else -width / 2)
                        else:
                            cx, cy = (length / 2 if x > 0 else -length / 2), y
                        shoes[round(cx, 6), round(cy + shift, 6)].append((x, y + shift))
                shared = [v for v in shoes.values() if len(v) == 2]
                self.assertEqual(len(shared), round(length / .5))
                self.assertTrue(all(len(v) <= 2 for v in shoes.values()))
                for a, b in shared:
                    self.assertAlmostEqual(abs(a[1] - b[1]), .08)
                    self.assertAlmostEqual(a[0], b[0])
                if (length, width) == (1, 1):
                    self.assertEqual(len(shoes), 14)
                    self.assertEqual(sum(len(v) == 1 for v in shoes.values()), 12)

    def test_shoe_requires_frame_but_has_no_invented_rating(self):
        shoe = mounting.deck_support_shoe()
        ports = {p['id']: p for p in shoe['ports']}
        self.assertEqual(ports['frame']['at'], [0, 0, -.06])
        self.assertFalse(ports['frame'].get('optional', False))
        self.assertTrue(ports['panel-a']['optional'] and ports['panel-b']['optional'])
        self.assertNotIn('rating', shoe)
        self.assertAlmostEqual(shoe['physical']['mass'], sum(p['mass'] for p in shoe['built_of']), places=6)


if __name__ == '__main__':
    unittest.main()
