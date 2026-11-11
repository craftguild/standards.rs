"""Refuse the frozen old-repository reference without silently rewriting it."""
import copy
import json
from pathlib import Path
import shutil

from entry_fixture import EntryFixture, ROOT, digest
from verify_execution import run
from verify_updates import snapshot


def verify(base, template, initializer):
    machine = copy.deepcopy(template.machine)
    machine.pop('command')  # Historical full CLI's identity grammar.
    machine['engine']['version'] = '0.0.1'
    fixture = EntryFixture(base / 'legacy-update', machine=machine)
    host = machine['target']
    stem = f'craftguild-standards-v0.0.1-{host}'
    binary = fixture.assets / stem
    bom = fixture.assets / (stem + '.bom.json')
    shutil.copy2(fixture.binary, binary)
    shutil.copy2(fixture.sbom, bom)
    prefix = 'https://github.com/craftguild/standards/releases/download/v0.0.1/'
    fixture.release['schema_version'] = 2
    fixture.release['hosts'][host] = {
        'binary': {'url': prefix + stem, 'sha256': digest(binary)},
        'sbom': {'url': prefix + stem + '.bom.json', 'sha256': digest(bom)}}
    old = ROOT / 'crates/craftguild-standards/tests/fixtures/initialization-v0.4.0'
    for source, destination in [('launcher.py', 'standards/launcher.py'),
                                ('common.mk', 'standards/make/common.mk'),
                                ('local-skill.md', '.agents/skills/craftguild-standardize/SKILL.md')]:
        path = fixture.root / destination
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(old / source, path)
    agents = (template.root / 'AGENTS.md').read_text().replace(
        'Update explicitly with `craftguild-standards init [--force]`; follow its dependency revision guidance.\n', '')
    (fixture.root / 'AGENTS.md').write_text(agents + '\nLegacy product contract\n')
    fixture.pin['schema_version'] = 2
    fixture.pin.pop('managed')
    fixture.pin['entries'] = {name: digest(fixture.entry / name) for name in ['launcher.py', 'make/common.mk']}
    fixture.save()
    run(fixture.root, ['make', 'setup'], fixture.env)
    run(fixture.root, ['make', 'prime'], fixture.env)
    before = snapshot(fixture.root)
    result = run(fixture.root, [initializer, 'init'], fixture.env, success=False)
    assert b'Existing fixed reference is incompatible' in result.stderr
    assert snapshot(fixture.root) == before
    print('PASS: old-repository reference refuses implicit upgrade and preserves owned files', flush=True)
