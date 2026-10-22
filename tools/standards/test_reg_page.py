"""Page verification must reject blank pages and browser failures."""
import contextlib
import io
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import reg


class PageVerification(unittest.TestCase):
    def check_page(self, dom, returncode=0, stderr=""):
        result = SimpleNamespace(stdout=dom, stderr=stderr, returncode=returncode)
        with patch.object(reg, "sh", return_value=result), contextlib.redirect_stdout(io.StringIO()):
            return reg.pagetest(["x:ship-rules:feature-intake"], "/tmp/registry.html")

    def test_real_detail_with_no_links_is_valid(self):
        self.assertEqual(self.check_page('<article id="detail"><h2>Rule</h2><p>Intent</p></article>'), 0)

    def test_blank_or_missing_detail_cannot_pass_from_surrounding_content(self):
        for dom in ('<header>Registry</header><article id="detail"></article><footer>Footer</footer>',
                    '<h2>Registry</h2>', '<article id="detail"><p> </p></article>'):
            with self.subTest(dom=dom):
                self.assertEqual(self.check_page(dom), 1)

    def test_browser_error_and_javascript_error_fail_even_with_content(self):
        dom = '<article id="detail"><h2>Rule</h2></article>'
        self.assertEqual(self.check_page(dom, returncode=1), 1)
        self.assertEqual(self.check_page(dom, stderr='Uncaught ReferenceError: missing'), 1)


if __name__ == "__main__":
    unittest.main()
