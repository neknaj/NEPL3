"""Keep architecture command coverage aligned with implemented portable crates."""
from pathlib import Path
import shlex
import unittest

from tools.evidence.records import specification
from tools.serialization.json import array, decode, object_value, string

ROOT = Path(__file__).resolve().parents[2]
FEATURES = {'nepl3-suite/doc-sentence', 'nepl3-suite/sentence-html'}


def implemented() -> set[str]:
    catalog = object_value(decode((ROOT / 'design/dependencies.json').read_bytes()))
    result: set[str] = set()
    for item in array(catalog['workspace']):
        entry = object_value(item)
        if entry['no_std'] is True and (ROOT / string(entry['path']) / 'Cargo.toml').is_file():
            result.add(string(entry['name']))
    return result


def check_packages(argv: tuple[str, ...], expected: set[str]) -> None:
    packages = [argv[i + 1] for i, value in enumerate(argv[:-1]) if value == '-p']
    if len(packages) != len(set(packages)) or set(packages) != expected:
        raise ValueError('portable package coverage differs from implemented inventory')
    if '--no-default-features' not in argv or '--features' not in argv:
        raise ValueError('explicit feature policy required')
    features = set(argv[argv.index('--features') + 1].split(','))
    if features != FEATURES:
        raise ValueError('both implemented suite adapters must be checked')


class ArchitectureSpecTests(unittest.TestCase):
    def test_spec_covers_implemented_portable_libraries(self) -> None:
        spec = specification(decode((ROOT / 'tools/evidence/specs/a01.json').read_bytes()))
        commands = {row.id: row for row in spec.commands}
        self.assertEqual(set(commands), {'rustc', 'cargo', 'target', 'repository', 'dependency-tests', 'portable-core-arm'})
        command = commands['portable-core-arm']
        check_packages(command.argv, implemented())
        self.assertIn('--lib', command.argv)
        self.assertEqual(command.argv[command.argv.index('--target') + 1], 'thumbv6m-none-eabi')
        for omitted in ['nepl3-math-tex', 'nepl3-math-mathml']:
            args = list(command.argv)
            index = args.index(omitted)
            del args[index - 1:index + 1]
            with self.assertRaises(ValueError):
                check_packages(tuple(args), implemented())

    def test_ci_covers_same_packages_on_both_compile_targets(self) -> None:
        targets: set[str] = set()
        for line in (ROOT / '.github/workflows/ci.yml').read_text().splitlines():
            if 'run: cargo check ' not in line or '--target ' not in line:
                continue
            args = tuple(shlex.split(line.split('run: ', 1)[1]))
            target = args[args.index('--target') + 1]
            if target in {'thumbv6m-none-eabi', 'wasm32-unknown-unknown'}:
                self.assertNotIn(target, targets)
                targets.add(target)
                check_packages(args, implemented())
        self.assertEqual(targets, {'thumbv6m-none-eabi', 'wasm32-unknown-unknown'})

    def test_ci_collection_is_opt_in_read_only_and_never_an_acceptance_decision(self) -> None:
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertTrue('collect_a01:' in ci, 'missing explicit collection input')
        concurrency = ci.split('concurrency:', 1)[1].split('\nenv:', 1)[0]
        self.assertIn("format('-a01-{0}', github.run_id)", concurrency)
        self.assertIn("inputs.collect_a01", concurrency)
        self.assertIn("github.event.pull_request.number || github.ref", concurrency)

        self.assertIn('default: false', ci.split('workflow_dispatch:', 1)[1].split('\npermissions:', 1)[0])
        job = ci.split('\n  a01-command-evidence:', 1)[1]
        self.assertIn("if: github.event_name == 'workflow_dispatch' && inputs.collect_a01", job)
        self.assertIn('persist-credentials: false', job)
        self.assertIn('rustup target add thumbv6m-none-eabi', job)
        self.assertIn('python -m tools.evidence.runner run tools/evidence/specs/a01.json dist/evidence/a01-command', job)
        self.assertIn('python -m tools.evidence.archive pack', job)
        self.assertIn('--source-revision "$GITHUB_SHA"', job)
        self.assertIn('set -o noclobber', job)
        self.assertIn("if: always() && steps.collect.outcome != 'skipped'", job)
        self.assertIn('if-no-files-found: error', job)
        self.assertIn('retention-days: 30', job)
        self.assertIn('acceptance_decision=false', job)
        self.assertNotIn('continue-on-error', job)
        self.assertNotIn('contents: write', job)
        self.assertNotIn('actions: write', job)
        self.assertNotIn('implementation-status.json', job)

        quality = ci.split('  quality:', 1)[1].split('  a01-command-evidence:', 1)[0]
        self.assertIn('a01-command-evidence]', quality)
        self.assertIn('test "$A01_RESULT" = success', quality)
        self.assertIn('test "$A01_RESULT" = skipped', quality)
