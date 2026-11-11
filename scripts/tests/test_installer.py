"""Run generated installers with real sh/hash and isolated process stubs."""

import os
from pathlib import Path
import shutil
import tempfile
import unittest

from installer_fixture import InstallerFixture


class InstallerTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory(prefix='standards-installer-')
        self.addCleanup(temporary.cleanup)
        return InstallerFixture(Path(temporary.name))

    def test_four_targets_aliases_latest_and_explicit_version(self):
        fixture = self.fixture()
        for system, arch, target in [
            ('Darwin', 'x86_64', 'x86_64-apple-darwin'),
            ('Darwin', 'arm64', 'aarch64-apple-darwin'),
            ('Linux', 'amd64', 'x86_64-unknown-linux-gnu'),
            ('Linux', 'aarch64', 'aarch64-unknown-linux-gnu'),
            ('Darwin', 'amd64', 'x86_64-apple-darwin'),
            ('Linux', 'arm64', 'aarch64-unknown-linux-gnu'),
        ]:
            fixture.env.update(OS=system, ARCH=arch)
            for version in [[], ['--version', '1.2.3']]:
                fixture.clear()
                result = fixture.run('--bindir', str(fixture.bindir), *version)
                self.assertIn(b'Add ', result.stdout)
                binary = fixture.bindir / 'craftguild-standards'
                self.assertEqual(binary.read_bytes(), b'#!/bin/sh\nexit 42\n')
                self.assertEqual(binary.stat().st_mode & 0o777, 0o755)
                log = fixture.log()
                curls = [entry for entry in log if entry[0] == 'curl']
                self.assertEqual(len(curls), 2 if version else 3)
                assets = [next(a for a in entry if a.startswith('https://')) for entry in curls[-2:]]
                self.assertTrue(assets[0].endswith('craftguild-standards-v1.2.3-' + target))
                self.assertTrue(all('/download/v1.2.3/' in url for url in assets))
                self.assertFalse(any(entry[0] == 'sudo' for entry in log))
                fixture.clean()
        if shutil.which('sha256sum') and shutil.which('shasum'):
            fixture.hash('shasum')
            fixture.run('--bindir', str(fixture.bindir))
            self.assertTrue(any(e[:3] == ['shasum', '-a', '256'] for e in fixture.log()))

    def test_validation_and_unsupported_hosts_have_no_side_effects(self):
        fixture = self.fixture()
        for args in [
            ['--version'], ['--bindir'], ['--bad'], ['--version', 'v1.2.3'],
            ['--version', '01.2.3'], ['--version', '1.2.3-beta'],
            ['--version', '1.2.3+build'], ['--version', '1.2.3\n1.2.3'],
            ['--bindir', ''], ['--bindir', 'relative'],
            ['--help', '--bindir', 'relative'],
            ['--version', '1.2.3', '--version', '1.2.3'],
            ['--uninstall', '--uninstall'], ['--help', '--help'],
            ['--bindir', '/a', '--bindir', '/b'],
            ['--uninstall', '--version', '1.2.3'],
        ]:
            fixture.clear()
            fixture.run(*args, success=False)
            self.assertEqual(fixture.log(), [])
        fixture.env.pop('HOME')
        fixture.run('--help')
        fixture.run(success=False)
        for system, arch in [('FreeBSD', 'arm64'), ('Linux', 'riscv64')]:
            fixture.clear()
            fixture.env.update(OS=system, ARCH=arch)
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertFalse(fixture.bindir.exists())
            self.assertFalse(any(e[0] == 'curl' for e in fixture.log()))

    def test_destination_precedence_spaces_and_home_only_for_default(self):
        fixture = self.fixture()
        fixture.run('--version', '1.2.3')
        self.assertTrue((fixture.home / '.local/bin/craftguild-standards').is_file())
        environment = fixture.root / 'environment bin'
        fixture.env['BINDIR'] = str(environment)
        fixture.env.pop('HOME')
        fixture.run('--version', '1.2.3')
        self.assertTrue((environment / 'craftguild-standards').is_file())
        fixture.env['PATH'] += ':' + str(fixture.bindir)
        result = fixture.run('--bindir', str(fixture.bindir))
        self.assertNotIn(b'Add ', result.stdout)
        fixture.env['BINDIR'] = ''
        fixture.run(success=False)
        fixture.run('--bindir', str(fixture.bindir))
        fixture.clean()

    def test_download_checksum_and_latest_failure_preserve_old_binary(self):
        fixture = self.fixture()
        fixture.bindir.mkdir()
        binary = fixture.bindir / 'craftguild-standards'
        binary.write_bytes(b'old binary')
        checksum = fixture.assets / 'SHA256SUMS-aarch64-apple-darwin.txt'
        correct = checksum.read_text()
        for content in ['', correct + correct, 'z' * 64 + correct[64:],
                        '0' * 64 + correct[64:], correct.replace('  ', ' extra '),
                        correct.replace('craftguild-', 'another-')]:
            checksum.write_text(content)
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertEqual(binary.read_bytes(), b'old binary')
            fixture.clean()
        checksum.write_text(correct)
        for fragment in ['craftguild-standards-v', 'SHA256SUMS', '/latest']:
            fixture.env['NETWORK_FAIL'] = fragment
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertEqual(binary.read_bytes(), b'old binary')
            fixture.clean()
        fixture.env.pop('NETWORK_FAIL')
        asset = fixture.assets / 'craftguild-standards-v1.2.3-aarch64-apple-darwin'
        asset.unlink()
        fixture.run('--bindir', str(fixture.bindir), success=False)
        fixture.publish(b'new verified binary')
        for tag in ['other-v1.2.3', 'v1.2.3-beta', 'v01.2.3', 'nested/v1.2.3']:
            fixture.env['LATEST'] = 'https://github.com/craftguild/standards.rs/releases/tag/' + tag
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertEqual(binary.read_bytes(), b'old binary')
        fixture.env['LATEST'] = 'https://github.com/another/repo/releases/tag/v1.2.3'
        fixture.run('--bindir', str(fixture.bindir), success=False)
        fixture.run('--version', '1.2.3', '--bindir', str(fixture.bindir))
        self.assertEqual(binary.read_bytes(), b'new verified binary')
        fixture.clean()

    def test_uninstall_without_network_host_or_hash_preserves_other_files(self):
        fixture = self.fixture()
        fixture.bindir.mkdir()
        binary = fixture.bindir / 'craftguild-standards'
        binary.write_text('installed')
        for name in ['curl', 'uname', 'sha256sum', 'shasum']:
            (fixture.tools / name).unlink(missing_ok=True)
        config = fixture.home / 'config'
        config.write_text('settings')
        other = fixture.bindir / 'other'
        other.write_text('other command')
        fixture.run('--uninstall', '--bindir', str(fixture.bindir))
        fixture.run('--uninstall', '--bindir', str(fixture.bindir))
        self.assertFalse(binary.exists())
        self.assertEqual(config.read_text(), 'settings')
        self.assertEqual(other.read_text(), 'other command')
        binary.mkdir()
        fixture.run('--uninstall', '--bindir', str(fixture.bindir), success=False)
        binary.rmdir()
        binary.symlink_to(other)
        fixture.run('--uninstall', '--bindir', str(fixture.bindir), success=False)
        self.assertEqual(other.read_text(), 'other command')
        self.assertEqual(fixture.log(), [])

    def test_no_sudo_when_writable_or_root_and_refuse_missing_sudo(self):
        fixture = self.fixture()
        (fixture.tools / 'sudo').unlink()
        fixture.env['UID_STUB'] = '0'
        fixture.run('--bindir', str(fixture.bindir))
        self.assertFalse(any(e[0] == 'sudo' for e in fixture.log()))
        fixture.run('--uninstall', '--bindir', str(fixture.bindir))
        if os.getuid() != 0:
            binary = fixture.bindir / 'craftguild-standards'
            binary.write_bytes(b'old')
            fixture.bindir.chmod(0o555)
            fixture.env['UID_STUB'] = '501'
            try:
                fixture.run('--bindir', str(fixture.bindir), success=False)
                self.assertEqual(binary.read_bytes(), b'old')
                fixture.run('--uninstall', '--bindir', str(fixture.bindir), success=False)
                self.assertEqual(binary.read_bytes(), b'old')
            finally:
                fixture.bindir.chmod(0o755)
        fixture.clean()

    def test_sudo_only_after_hash_preserves_argv_and_cleans_failed_staging(self):
        if os.getuid() == 0:
            self.skipTest('Permission boundary requires a non-root test host')
        fixture = self.fixture()
        fixture.bindir.mkdir()
        fixture.env['LOCKED'] = str(fixture.bindir)
        binary = fixture.bindir / 'craftguild-standards'
        binary.write_bytes(b'old')
        for failure in ['mkdir', 'mktemp', 'cp', 'chmod', 'mv']:
            fixture.clear()
            fixture.bindir.chmod(0o555)
            fixture.env['SUDO_FAIL'] = failure
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertEqual(binary.read_bytes(), b'old')
            fixture.bindir.chmod(0o755)
            fixture.clean()
            log = fixture.log()
            names = [e[0] for e in log]
            self.assertLess(next(i for i,n in enumerate(names) if n in {'sha256sum','shasum'}), names.index('sudo'))
            for entry in log:
                if entry[0] == 'sudo':
                    self.assertIn(entry[1], {'mkdir','mktemp','cp','chmod','mv','rm'})
            if failure in {'cp','chmod','mv'}:
                self.assertEqual(log[-1][:3], ['sudo','rm','-f'])
        fixture.env.pop('SUDO_FAIL')
        fixture.bindir.chmod(0o555)
        fixture.run('--bindir', str(fixture.bindir))
        log = [e for e in fixture.log() if e[0] == 'sudo'][-5:]
        self.assertEqual([e[1] for e in log], ['mkdir','mktemp','cp','chmod','mv'])
        self.assertEqual(log[0], ['sudo','mkdir','-p',str(fixture.bindir)])
        self.assertEqual(log[-1][-1], str(binary))
        fixture.bindir.chmod(0o555)
        fixture.run('--uninstall', '--bindir', str(fixture.bindir))
        self.assertEqual(fixture.log()[-1], ['sudo','rm','-f',str(binary)])
        fixture.bindir.chmod(0o755)
        binary.write_bytes(b'old')
        fixture.bindir.chmod(0o555)
        fixture.env['SIGNAL_AT'] = 'cp'
        fixture.run('--bindir', str(fixture.bindir), success=False)
        self.assertEqual(binary.read_bytes(), b'old')
        fixture.bindir.chmod(0o755)
        fixture.clean()
        # Missing descendants of a protected parent use sudo only for destination work.
        protected = fixture.bindir
        fixture.bindir = protected / 'new bin'
        protected.chmod(0o555)
        fixture.env.pop('SIGNAL_AT')
        fixture.clear()
        try:
            fixture.run('--bindir', str(fixture.bindir))
            self.assertEqual(fixture.log()[-1][-1], str(fixture.bindir / 'craftguild-standards'))
            fixture.clean()
        finally:
            protected.chmod(0o755)


if __name__ == '__main__':
    unittest.main()
