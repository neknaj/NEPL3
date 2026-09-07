import unittest
import contract


class ContractMigration(unittest.TestCase):
    def test_current_source_and_explicit_code(self):
        source = contract.SOURCE.read_text(encoding="utf-8")
        result = contract.generate(source)
        self.assertEqual(result, contract.TARGET.read_text(encoding="utf-8"))
        self.assertIn('sentence cons text "list', result)
        self.assertIn('cons code "cons"', result)
        self.assertEqual(result.count("cons section contract_"), 6)

    def test_unsupported_features_fail_before_generation(self):
        source = contract.SOURCE.read_text(encoding="utf-8")
        for addition in [
            "first  \nsecond", "x &amp; y", "    indented code", "Heading\n=======",
            "[label](page.md)", "![alt](image.png)", "**strong**", "*emphasis*",
            "<i>html</i>", "```rust\ncode\n```", "- [x] task", "1. ordered",
            "| a | b |", "$x$", "~~deleted~~", "` padded `", "a\\\nb",
            "## new heading", "text\n  continuation", "---",
        ]:
            with self.subTest(addition=addition), self.assertRaises(ValueError):
                contract.generate(source + "\n" + addition + "\n")

    def test_plain_soft_break_and_literal_escape(self):
        source = contract.SOURCE.read_text(encoding="utf-8")
        self.assertIn('"first second"', contract.generate(source + "\nfirst\nsecond\n"))
        self.assertEqual(contract.sentence('a {b}'), '"a \\{b\\}"')


if __name__ == "__main__":
    unittest.main()
