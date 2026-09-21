"""Reject checkout changes after generation; never stage or remove them."""

import subprocess
import sys


def main():
    result = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all", "--ignore-submodules=none"],
        stdout=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        return result.returncode
    if result.stdout:
        print("Checkout changed (index, working tree, or untracked files):", flush=True)
        sys.stdout.buffer.write(result.stdout)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
