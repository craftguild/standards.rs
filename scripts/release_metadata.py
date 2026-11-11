#!/usr/bin/env python3
"""Combine exact verified host deliveries into one immutable release index."""

import argparse
import hashlib
import json
from pathlib import Path
import tempfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def merge(source, destination):
    if destination.exists() or destination.is_symlink():
        raise ValueError('Release index destination must be fresh')
    records = sorted(source.rglob('craftguild-release-v*-*.json'))
    if not records:
        raise ValueError('No host release metadata')
    combined = None
    for path in records:
        if path.is_symlink():
            raise ValueError('Symlink in host release metadata')
        record = json.loads(path.read_bytes())
        if (set(record) != {'schema_version', 'identity', 'hosts'}
                or record['schema_version'] != 3 or not record['hosts']):
            raise ValueError('Incompatible host release metadata')
        identity = record['identity']
        if (set(identity) != {'version', 'revision', 'baseline', 'protocol', 'entry_protocol'}
                or identity['baseline'] != '0.1.0'
                or identity['protocol'] != 'craftguild-execution-1'
                or identity['entry_protocol'] != 'craftguild-entry-1'
                or len(identity['revision']) != 40
                or any(c not in '0123456789abcdef' for c in identity['revision'])):
            raise ValueError('Invalid host release identity')
        version = identity['version']
        if len(version.split('.')) != 3 or any(
                not p.isascii() or not p.isdecimal() or (len(p) > 1 and p[0] == '0')
                or int(p) > 2**64 - 1 for p in version.split('.')):
            raise ValueError('Expected stable release version')
        if combined is None:
            combined = {'schema_version': 3, 'identity': identity, 'hosts': {}}
        if combined['identity'] != identity:
            raise ValueError('Host deliveries differ in source/version/compatibility')
        for host, assets in record['hosts'].items():
            if host not in {'aarch64-apple-darwin', 'x86_64-apple-darwin',
                            'aarch64-unknown-linux-gnu', 'x86_64-unknown-linux-gnu'}:
                raise ValueError('Unsupported release host')
            if host in combined['hosts'] or set(assets) != {'initializer', 'engine'}:
                raise ValueError('Duplicate host or invalid artifact roles')
            for role, command in [('initializer', 'craftguild-standards'),
                                  ('engine', 'craftguild-standards-engine')]:
                pair = assets[role]
                if set(pair) != {'binary', 'sbom'}:
                    raise ValueError('Invalid role asset ownership')
                stem = f'{command}-v{version}-{host}'
                for kind, suffix in [('binary', ''), ('sbom', '.bom.json')]:
                    asset = pair[kind]
                    name = stem + suffix
                    expected_url = f'https://github.com/craftguild/standards.rs/releases/download/v{version}/{name}'
                    artifact = path.parent / name
                    if (set(asset) != {'url', 'sha256'} or asset['url'] != expected_url
                            or artifact.is_symlink() or not artifact.is_file()
                            or digest(artifact) != asset['sha256']):
                        raise ValueError('Host release role asset URL/bytes differ')
            combined['hosts'][host] = assets
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='release-index-', dir=destination.parent) as name:
        stage = Path(name) / 'index'
        stage.mkdir()
        metadata = stage / f'craftguild-release-v{combined["identity"]["version"]}.json'
        metadata.write_text(json.dumps(combined, sort_keys=True, indent=2) + '\n')
        (stage / 'SHA256SUMS-release.txt').write_text(f'{digest(metadata)}  {metadata.name}\n')
        stage.rename(destination)
    return combined


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    merge(args.source, args.output)
