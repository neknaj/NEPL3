"""Font polling uses a host deadline even when page scripts cannot run timers."""
import unittest
from typing import final
from unittest.mock import patch

from fonts import wait_for_fonts


@final
class Observation:
    def __init__(self, loaded: bool) -> None:
        self.loaded = loaded
        self.calls = 0

    def evaluate(self, expression: str) -> object:
        if 'document.fonts' not in expression:
            raise AssertionError('Expected a synchronous font observation')
        self.calls += 1
        return self.loaded


class FontDeadlineTests(unittest.TestCase):
    def test_loaded_fonts_finish_without_page_timers(self) -> None:
        page = Observation(True)
        with patch('fonts.time.monotonic', side_effect=[0, 0]):
            wait_for_fonts(page)
        self.assertEqual(page.calls, 1)

    def test_unavailable_fonts_fail_at_host_deadline(self) -> None:
        page = Observation(False)
        with patch('fonts.time.monotonic', side_effect=[0, 0, 31]), patch('fonts.time.sleep'):
            with self.assertRaisesRegex(AssertionError, 'within 30 seconds'):
                wait_for_fonts(page)
        self.assertEqual(page.calls, 1)
