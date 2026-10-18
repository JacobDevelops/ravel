#!/usr/bin/env python3
"""Reject a stale modern agent before an explicit official legacy launch."""
from pathlib import Path
import sys
import zipfile


def main():
    agent = Path(sys.argv[1])
    with zipfile.ZipFile(agent) as archive:
        manifest = archive.read('META-INF/MANIFEST.MF').decode('utf-8')
        if 'Ravel-Legacy-Adapter: true' not in manifest.splitlines():
            raise SystemExit('Agent is not marked as a generated legacy adapter; rebuild for 1.12.2')
        if 'Automatic-Module-Name: dev.ravel.agent' not in manifest.splitlines():
            raise SystemExit('Legacy JNI requires the named dev.ravel.agent module')
        for entry in ('dev/ravel/jvm/Hook.class', 'dev/ravel/jvm/LegacyRuntime.class'):
            if entry not in archive.namelist():
                raise SystemExit(f'Legacy agent is missing {entry}')


if __name__ == '__main__':
    main()
