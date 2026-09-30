#!/usr/bin/env python3
"""Run one measured process in a fresh wrapper and capture Linux peak RSS."""
from pathlib import Path
import resource
import subprocess
import sys

completed = subprocess.run(sys.argv[2:])
Path(sys.argv[1]).write_text(str(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss) + "\n")
raise SystemExit(completed.returncode)
