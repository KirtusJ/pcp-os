#!/usr/bin/env python3
"""Start PCP OS from wherever this packet was extracted. No installation, no shell script, no path setup.

    Linux, macOS:   python3 /path/to/PCP_OS_Python/pcp_os.py <command>
    Windows:        py \\path\\to\\PCP_OS_Python\\pcp_os.py <command>
"""
import sys
from pathlib import Path

if sys.version_info < (3, 9):
    sys.stderr.write("PCP OS needs Python 3.9 or later; this is %s\n" % sys.version.split()[0])
    sys.exit(5)
sys.dont_write_bytecode = True                  # the packet stays exactly as released
sys.path.insert(0, str(Path(__file__).resolve().parent))
from pcpos.cli import main                      # noqa: E402

sys.exit(main())
