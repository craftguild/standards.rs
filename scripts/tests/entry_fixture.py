"""Isolated flat release assets, a transport stub, and a fixed consumer entry."""

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def target():
    arch = {'arm64': 'aarch64', 'aarch64': 'aarch64', 'x86_64': 'x86_64', 'amd64': 'x86_64'}[platform.machine()]
    return arch + '-' + {'Darwin': 'apple-darwin', 'Linux': 'unknown-linux-gnu'}[platform.system()]


class EntryFixture:
    def __init__(self, base, binary=None, sbom=None, machine=None):
        self.base = base
        self.root = base / 'consumer'
        self.entry = self.root / 'standards'
        (self.entry / 'make').mkdir(parents=True)
        shutil.copy2(ROOT / 'templates/launcher.py', self.entry / 'launcher.py')
        shutil.copy2(ROOT / 'templates/make/common.mk', self.entry / 'make/common.mk')
        self.machine = machine or {'command': 'craftguild-standards-engine', 'engine': {'version': '7.8.9', 'revision': 'a' * 40,
                                             'baseline': '0.1.0', 'protocol': 'craftguild-execution-1',
                                             'entry_protocol': 'craftguild-entry-1'},
                                   'target': target(), 'clean': True}
        self.assets = base / 'assets'
        self.assets.mkdir()
        version = self.machine['engine']['version']
        name = f'craftguild-standards-engine-v{version}-{self.machine["target"]}'
        self.binary = self.assets / name
        self.sbom = self.assets / (name + '.bom.json')
        self.sbom.write_bytes(sbom or b'{"fixture": "sbom"}\n')
        if binary:
            shutil.copy2(binary, self.binary)
        else:
            self.fake_binary(self.machine)
        prefix = f'https://github.com/craftguild/standards.rs/releases/download/v{version}/'
        engine = {'binary': {'url': prefix + name, 'sha256': digest(self.binary)},
                  'sbom': {'url': prefix + name + '.bom.json', 'sha256': digest(self.sbom)}}
        init_name = f'craftguild-standards-v{version}-{self.machine["target"]}'
        # A distinct artifact with its own bytes; never executable as an engine.
        initializer = {}
        for kind, suffix, data in [('binary', '', b'initializer fixture'), ('sbom', '.bom.json', b'initializer sbom')]:
            path = self.assets / (init_name + suffix)
            path.write_bytes(data)
            initializer[kind] = {'url': prefix + path.name, 'sha256': digest(path)}
        self.release = {'schema_version': 3, 'identity': self.machine['engine'],
                        'hosts': {self.machine['target']: {'initializer': initializer, 'engine': engine}}}
        self.pin = {'schema_version': 3, 'release': self.release,
                    'managed': {name: 'b' * 64 for name in ['.agents/skills/craftguild-standardize/SKILL.md', 'AGENTS.md']},
                    'entries': {name: digest(self.entry / name) for name in ['launcher.py', 'make/common.mk']}}
        self.save()
        (self.root / 'Makefile').write_text('include standards/make/common.mk\n')
        self.tools = base / 'tools'
        self.tools.mkdir()
        self.log = base / 'transport.log'
        curl = self.tools / 'curl'
        curl.write_text('#!' + sys.executable + '\n' + '''import os,pathlib,sys,time
args=sys.argv[1:]
url=args[-1]
with open(os.environ['STUB_LOG'], 'a') as log: log.write(url+'\\n')
if os.environ.get('STUB_FAIL'): sys.exit(22)
if os.environ.get('STUB_BLOCK'):
    pathlib.Path(os.environ['STUB_BLOCK']).write_text('ready')
    time.sleep(30)
data=(pathlib.Path(os.environ['STUB_ASSETS']) / url.rsplit('/',1)[-1]).read_bytes()
if '--output' in args:
    pathlib.Path(args[args.index('--output')+1]).write_bytes(data)
else: sys.stdout.buffer.write(data)
''')
        curl.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ['PATH'],
                        HOME=str(base / 'home'), STUB_ASSETS=str(self.assets), STUB_LOG=str(self.log),
                        RUSTUP_HOME=os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup')),
                        CARGO_HOME=os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
        (base / 'home').mkdir()

    def fake_binary(self, machine, embedded=None):
        self.binary.write_text('#!' + sys.executable + '\nimport pathlib,sys\n'
                               'if sys.argv[1] == "engine-identity": print(' + repr(json.dumps(machine)) + ')\n'
                               'elif sys.argv[1] == "sbom": sys.stdout.buffer.write(' + repr(embedded or self.sbom.read_bytes()) + ')\n'
                               'elif sys.argv[1] == "prime-context": print("# Matching fixture context")\n'
                               'else: pathlib.Path("dispatched").write_text(" ".join(sys.argv[1:]))\n')
        self.binary.chmod(0o755)

    def save(self):
        (self.entry / 'engine.json').write_text(json.dumps(self.pin, indent=2) + '\n')
        version = self.machine['engine']['version']
        (self.assets / f'craftguild-release-v{version}.json').write_text(json.dumps(self.release))

    @property
    def cache(self):
        return self.root / '.craftguild/engine' / digest(self.entry / 'engine.json') / self.machine['target']

    def calls(self):
        return self.log.read_text().splitlines() if self.log.exists() else []
