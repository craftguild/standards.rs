"""Isolated real consumers for source-development execution acceptance."""

import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TRANSPORT = {}


def run(root, argv, env=None, success=True):
    environment = dict(os.environ)
    for key in ['ARTIFACT', 'TARGET', 'PROFILE', 'TOOLCHAIN', 'FEATURES',
                'NO_DEFAULT_FEATURES', 'CARGO_TARGET_DIR', 'CRAFTGUILD_PREPARED']:
        environment.pop(key, None)
    environment.update(TRANSPORT)
    environment.update(env or {})
    result = subprocess.run([str(a) for a in argv], cwd=root, env=environment,
                            capture_output=True, timeout=300)
    if success != (result.returncode == 0):
        raise AssertionError(f'{argv}: {result.returncode}\n{result.stderr.decode()}')
    return result


def consumer(root, name, entry, standard=False):
    shutil.copytree(ROOT / 'examples' / name, root,
                    ignore=shutil.ignore_patterns('.craftguild', 'target', 'node_modules', '__pycache__'))
    shutil.copytree(entry, root / 'standards')
    shutil.copytree(ROOT / 'schemas', root / 'schemas')
    for library in ['craftguild-cli', 'craftguild-cli-build', 'craftguild-core']:
        shutil.copytree(ROOT / 'crates' / library, root / 'crates' / library)
    manifest = (root / 'Cargo.toml').read_text().replace('[workspace]\n', '')
    manifest = re.sub(r'\{ git = "https://github.com/craftguild/standards\.rs", rev = "[a-f0-9]+" \}',
                      lambda m: m.group(0), manifest)
    for library in ['craftguild-cli', 'craftguild-cli-build']:
        manifest = re.sub(library + r' = \{ git = [^\n]+',
                          library + ' = { path = "crates/' + library + '" }', manifest)
    manifest += '\n[features]\noptional-part=[]\n'
    workspace = (ROOT / 'Cargo.toml').read_text().split('[workspace.package]', 1)[1]
    manifest += '\n[workspace]\nmembers=["crates/*"]\nresolver="2"\n[workspace.package]' + workspace
    (root / 'Cargo.toml').write_text(manifest)
    shutil.copyfile(ROOT / 'Cargo.lock', root / 'Cargo.lock')
    run(root, ['cargo', 'generate-lockfile', '--offline'])
    config = (root / 'craftguild.toml').read_text().replace('schema_version = 1', 'schema_version = 2', 1)
    if 'input_dirs = ["web/src"]' in config:
        config = config.replace('input_dirs = ["web/src"]', 'input_dirs = ["web/src", "schemas"]')
    else:
        config = config.replace('inputs = ["assets/banner.txt"]', 'inputs = ["assets/banner.txt"]\ninput_dirs = ["schemas"]')
    if standard:
        config = config.replace('schema_version = 2', 'schema_version = 2\nrust_output_layout = "standard"', 1)
    (root / 'craftguild.toml').write_text(config)
    (root / 'Makefile').write_text('include standards/make/common.mk\n')


def receipt(root, command):
    return json.loads((root / f'.craftguild/{command}/receipt.json').read_text())


def offline(root, command):
    record = receipt(root, command)
    binary = Path(record['context']['output'])
    isolated = root / 'offline'
    isolated.mkdir(exist_ok=True)
    copied = isolated / command
    shutil.copy2(binary, copied)
    (isolated / 'craftguild.toml').write_text('invalid [')
    env = {'PATH': ''}
    assert run(isolated, [copied, 'sbom'], env).stdout == (root / record['prepared']['sbom']).read_bytes()
    info = json.loads(run(isolated, [copied, '--machine-version'], env).stdout)
    assert info == record['prepared']['build']
    bom = json.loads((root / record['prepared']['sbom']).read_text())
    assert 'figlet-rs' not in {component['name'] for component in bom['components']}
    assert {p['name'] for p in bom['metadata']['properties']} == {'craftguild:build', 'craftguild:features'}
    assert run(isolated, [copied], env).stdout == run(isolated, [copied, '--help'], env).stdout
    assert run(isolated, [copied, '--version'], env).returncode == 0
    return record


def verify(entry, environment=None):
    TRANSPORT.clear()
    TRANSPORT.update(environment or {})
    with tempfile.TemporaryDirectory(prefix='craftguild-real-consumers-') as temporary:
        base = Path(temporary).resolve()
        rust = base / 'rust'
        consumer(rust, 'rust-only', entry, standard=True)
        # Ordinary install cannot acquire the pinned engine or create a cache.
        refused = run(rust, ['make', 'install', 'DESTDIR=' + str(base / 'stage')], success=False)
        assert b'engine cache is missing' in refused.stderr
        assert not (rust / '.craftguild/engine').exists()
        run(rust, ['make', 'setup'])
        run(rust, ['make', 'build', 'PROFILE=debug', 'FEATURES=optional-part', 'CARGO_TARGET_DIR=products'])
        first = offline(rust, 'sample')
        engine = next((rust / '.craftguild/engine').glob('*/*/craftguild-standards-engine'))
        run(rust, [sys.executable, 'standards/launcher.py', 'build', '--profile', 'debug',
                   '--features', 'optional-part', '--target-dir', 'products'])
        direct = receipt(rust, 'sample')
        assert first['context'] == direct['context']
        assert first['prepared']['sbom_sha256'] == direct['prepared']['sbom_sha256']
        run(rust, [engine, 'build', '--profile', 'debug', '--features', 'optional-part', '--target-dir', 'products'])
        ci = receipt(rust, 'sample')
        assert ci['context'] == first['context']
        assert ci['prepared']['sbom_sha256'] == first['prepared']['sbom_sha256']
        run(rust, [engine, 'install', '--profile', 'debug', '--features', 'optional-part', '--target-dir', 'products',
                   '--prefix', '/opt/sample', '--destdir', str(base / 'stage')], {'PATH': '', 'RUSTC': 'missing'})
        assert (base / 'stage/opt/sample/bin/sample').read_bytes() == Path(ci['context']['output']).read_bytes()
        run(rust, ['make', 'build', 'PROFILE=release'])
        assert offline(rust, 'sample')['prepared']['build']['profile'] == 'release'
        # Parent Make defaults must not override a child install's selected PREFIX.
        with (rust / 'Makefile').open('a') as makefile:
            makefile.write('\n.PHONY: install-acceptance\ninstall-acceptance:\n'
                           '\t$(MAKE) install PREFIX=/opt/nested DESTDIR="$(CURDIR)/nested-stage"\n')
        # Makefile is not a sealed input in this small consumer.
        run(rust, ['make', 'install-acceptance'])
        assert (rust / 'nested-stage/opt/nested/bin/sample').read_bytes() == Path(receipt(rust, 'sample')['context']['output']).read_bytes()
        with (rust / 'craftguild.toml').open('a') as config_file:
            config_file.write('\n[artifacts.build]\nprofile="debug"\ntarget_dir="configured-output"\n')
        run(rust, ['make', 'build'])
        configured = receipt(rust, 'sample')
        assert configured['context']['profile'] == 'debug'
        assert Path(configured['context']['target_dir']) == rust / 'configured-output'
        run(rust, [sys.executable, 'standards/launcher.py', 'build'])
        assert receipt(rust, 'sample')['context'] == configured['context']
        run(rust, [engine, 'build'])
        assert receipt(rust, 'sample')['context'] == configured['context']
        run(rust, ['make', 'install', 'PREFIX=/opt/configured', 'DESTDIR=' + str(base / 'configured-stage')])
        assert (base / 'configured-stage/opt/configured/bin/sample').read_bytes() == Path(configured['context']['output']).read_bytes()
        run(rust, ['make', 'build'], {'PROFILE': 'release', 'CARGO_TARGET_DIR': 'environment-output'})
        assert receipt(rust, 'sample')['context']['profile'] == 'release'
        assert Path(receipt(rust, 'sample')['context']['target_dir']) == rust / 'environment-output'
        run(rust, ['make', 'build', 'PROFILE=debug', 'CARGO_TARGET_DIR=make-output'])
        assert receipt(rust, 'sample')['context']['profile'] == 'debug'
        assert Path(receipt(rust, 'sample')['context']['target_dir']) == rust / 'make-output'
        run(rust, ['make', 'build', 'PROFILE=release', 'CARGO_TARGET_DIR=make-output',
                   'BUILD_ARGS=--profile debug --target-dir cli-output'])
        override = receipt(rust, 'sample')
        assert override['context']['profile'] == 'debug'
        assert Path(override['context']['target_dir']) == rust / 'cli-output'
        run(rust, ['make', 'install', 'INSTALL_ARGS=--target-dir cli-output',
                   'PREFIX=/opt/override', 'DESTDIR=' + str(base / 'override-stage')])
        assert (base / 'override-stage/opt/override/bin/sample').read_bytes() == Path(override['context']['output']).read_bytes()
        print('PASS: real Rust consumer, first/cache entry, Make/CLI/CI, profiles/features/output, offline and copy-only install', flush=True)

        web = base / 'embedded'
        consumer(web, 'embedded-web', entry, standard=True)
        modules = ROOT / 'examples/embedded-web/web/node_modules'
        if not modules.is_dir():
            raise ValueError('Install locked example dependencies before this acceptance: npm ci --prefix examples/embedded-web/web')
        shutil.copytree(modules, web / 'web/node_modules', symlinks=True)
        config = (web / 'craftguild.toml').read_text()
        config += '''
[[artifacts]]
id="frontend"
command="example-ui"
project="example-ui"
kind="frontend"
manifest="web/package.json"
lockfile="web/package-lock.json"
package="example-ui"
input_dirs=["web/src"]
[artifacts.bundle]
root="web"
lockfile="web/package-lock.json"
metafile="web/metafile.json"
directory="web/dist"
command=["npm","--prefix","web","run","build"]
'''
        (web / 'craftguild.toml').write_text(config)
        run(web, ['make', 'setup'])
        run(web, ['make', 'build', 'ARTIFACT=frontend', 'PROFILE=debug'], {'RUSTC': str(base / 'missing-rustc')})
        front = receipt(web, 'example-ui')
        assert front['prepared']['build']['compiler'].startswith('node ')
        front_bom = json.loads((web / front['prepared']['sbom']).read_text())
        assert {c['name'] for c in front_bom['components']} == {'is-number'}
        run(web, ['make', 'build', 'ARTIFACT=backend', 'PROFILE=debug'])
        embedded = offline(web, 'embedded-demo')
        bom = json.loads((web / embedded['prepared']['sbom']).read_text())
        names = {c['name'] for c in bom['components']}
        assert 'is-number' in names and 'craftguild-cli' in names
        assert not names & {'esbuild', 'typescript', 'picocolors', 'craftguild-cli-build', 'craftguild-core', 'figlet-rs'}
        actual = run(web, [embedded['context']['output'], 'asset']).stdout
        assert actual == (web / 'web/dist/app.js').read_bytes()
        print('PASS: actual npm/esbuild standalone and embedded Web, runtime-only SBOM and no frontend Rust probe', flush=True)


if __name__ == '__main__':
    verify(Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / '.craftguild/entry-development').resolve())
