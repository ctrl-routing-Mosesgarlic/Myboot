#!/usr/bin/env python3
"""MyBoot management entry point. Run `./manage.py --help`."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from pyboot.cli import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
