"""Authenticated transport exercises the same generated installer and placement."""

from pathlib import Path
import tempfile
import unittest

from installer_fixture import InstallerFixture


class AuthenticatedInstallerTests(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory(prefix='standards-private-installer-')
        self.addCleanup(temporary.cleanup)
        fixture = InstallerFixture(Path(temporary.name), transport='github-cli')
        fixture.stub('gh')
        (fixture.tools / 'curl').unlink()
        return fixture

    def test_stored_login_and_token_install_pin_host_tag_and_exact_assets(self):
        fixture = self.fixture()
        binary = fixture.bindir / fixture.command
        for credential in ['GH_STORED_AUTH', 'GH_TOKEN']:
            fixture.env[credential] = 'fixture-secret'
            fixture.env.update(GH_HOST='untrusted.invalid', GH_DEBUG='api', DEBUG='1')
            for selector in [[], ['--version', '1.2.3']]:
                fixture.clear()
                result = fixture.run('--bindir', str(fixture.bindir), *selector)
                self.assertEqual(binary.read_bytes(), b'#!/bin/sh\nexit 42\n')
                calls = [e for e in fixture.log() if e[0] == 'gh']
                self.assertEqual(len(calls), 1 if selector else 2)
                for call in calls:
                    self.assertEqual(call[call.index('--repo') + 1], 'github.com/craftguild/standards.rs')
                self.assertEqual(calls[-1][1:4], ['release', 'download', 'v1.2.3'])
                self.assertEqual(
                    [calls[-1][i + 1] for i, value in enumerate(calls[-1]) if value == '--pattern'],
                    ['craftguild-standards-v1.2.3-aarch64-apple-darwin',
                     'SHA256SUMS-aarch64-apple-darwin.txt'])
                self.assertNotIn(b'fixture-secret', result.stdout + result.stderr + fixture.events.read_bytes())
                fixture.clean()
            fixture.env.pop(credential)
        for tool in ['gh', 'uname', 'sha256sum', 'shasum']:
            (fixture.tools / tool).unlink(missing_ok=True)
        fixture.clear()
        fixture.run('--uninstall', '--bindir', str(fixture.bindir))
        self.assertFalse(binary.exists())
        self.assertEqual(fixture.log(), [])

    def test_auth_failure_partial_download_and_bad_latest_preserve_old_binary(self):
        fixture = self.fixture()
        fixture.bindir.mkdir()
        binary = fixture.bindir / fixture.command
        binary.write_bytes(b'old binary')
        for failure in [None, 'view', 'SHA256SUMS']:
            fixture.env.pop('GH_TOKEN', None)
            fixture.env.pop('GH_FAIL', None)
            if failure:
                fixture.env.update(GH_TOKEN='fixture-secret', GH_FAIL=failure)
            result = fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertNotIn(b'fixture-secret', result.stdout + result.stderr)
            self.assertEqual(binary.read_bytes(), b'old binary')
            fixture.clean()
        fixture.env.pop('GH_FAIL')
        for tag in ['other-v1.2.3', 'v1.2.3-beta', 'v01.2.3', 'nested/v1.2.3']:
            fixture.env['GH_LATEST_TAG'] = tag
            fixture.clear()
            fixture.run('--bindir', str(fixture.bindir), success=False)
            self.assertFalse(any(e[1:3] == ['release', 'download'] for e in fixture.log()))
            self.assertEqual(binary.read_bytes(), b'old binary')
        (fixture.tools / 'gh').unlink()
        fixture.run('--version', '1.2.3', '--bindir', str(fixture.bindir), success=False)
        self.assertEqual(binary.read_bytes(), b'old binary')

    def test_authenticated_bytes_still_require_checksum_verification(self):
        fixture = self.fixture()
        fixture.env['GH_TOKEN'] = 'fixture-secret'
        fixture.bindir.mkdir()
        binary = fixture.bindir / fixture.command
        binary.write_bytes(b'old binary')
        asset = fixture.assets / 'craftguild-standards-v1.2.3-aarch64-apple-darwin'
        asset.write_bytes(b'tampered bytes')
        result = fixture.run('--version', '1.2.3', '--bindir', str(fixture.bindir), success=False)
        self.assertIn(b'SHA-256 mismatch', result.stderr)
        self.assertEqual(binary.read_bytes(), b'old binary')
        fixture.clean()

    def test_successful_partial_pattern_match_cannot_replace_the_old_binary(self):
        fixture = self.fixture()
        fixture.env['GH_TOKEN'] = 'fixture-secret'
        fixture.bindir.mkdir()
        binary = fixture.bindir / fixture.command
        binary.write_bytes(b'old binary')
        for missing in ['craftguild-standards-v1.2.3-aarch64-apple-darwin',
                        'SHA256SUMS-aarch64-apple-darwin.txt']:
            fixture.publish(b'#!/bin/sh\nexit 42\n')
            (fixture.assets / missing).unlink()
            result = fixture.run('--version', '1.2.3', '--bindir', str(fixture.bindir), success=False)
            self.assertNotIn(b'GitHub Release asset download failed', result.stderr)
            self.assertNotIn(b'fixture-secret', result.stdout + result.stderr)
            self.assertEqual(binary.read_bytes(), b'old binary')
            fixture.clean()


if __name__ == '__main__':
    unittest.main()
