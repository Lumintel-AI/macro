#!/usr/bin/env python3
"""Inspect every runtime layer before registry publication, without printing secrets."""
import json
import pathlib
import re
import subprocess
import sys
import tarfile
import tempfile

image, report = sys.argv[1:]
# File-type databases contain PEM headers as detection strings. Require a full
# key block, and token boundaries so binary character tables are not credentials.
secret = re.compile(
    rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----\s+'
    rb'[A-Za-z0-9+/=\r\n]{32,32768}-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'
    rb'|(?<![A-Za-z0-9])AKIA[0-9A-Z]{16}(?![A-Za-z0-9])'
    rb'|(?<![A-Za-z0-9])gh[pousr]_[A-Za-z0-9]{30,255}(?![A-Za-z0-9])'
)
# Distro libraries that embed published self-test keys, not deployment secrets.
# GnuTLS ships fixed FIPS 140 known-answer-test private keys inside libgnutls.
known_test_key_libraries = re.compile(r'^usr/lib/[a-z0-9_-]+-linux-gnu/libgnutls\.so\.30(?:\.[0-9]+)*$')
findings = []
config = subprocess.check_output(["docker", "image", "inspect", image])
if secret.search(config):
    findings.append({"reason": "secret in image config"})
files = 0
with tempfile.TemporaryFile() as archive:
    subprocess.run(['docker', 'save', image], stdout=archive, check=True)
    archive.seek(0)
    with tarfile.open(fileobj=archive, mode='r|') as outer:
        for entry in outer:
            if not entry.isfile():
                continue
            stream = outer.extractfile(entry)
            if entry.name.endswith('.json'):
                if secret.search(stream.read()):
                    findings.append({'layer': entry.name, 'reason': 'secret in configuration'})
            elif entry.name.endswith('/layer.tar') or '/blobs/sha256/' in '/'+entry.name:
                try:
                    with tarfile.open(fileobj=stream, mode='r|*') as layer:
                        for member in layer:
                            if not member.isfile():
                                continue
                            files += 1
                            name = member.name.lstrip('./')
                            path = pathlib.PurePosixPath(name)
                            if name.startswith('app/') and (path.name == '.env' or path.name.startswith('.env.') or path.suffix in ('.env', '.pem', '.key') or '/.git/' in name or '/.aws/' in name or '/.ssh/' in name):
                                findings.append({'path': name, 'reason': 'forbidden configuration path'})
                            content = layer.extractfile(member)
                            overlap = b''
                            while chunk := content.read(1024 * 1024):
                                if secret.search(overlap + chunk) and not known_test_key_libraries.match(name):
                                    findings.append({'path': name, 'reason': 'credential signature'})
                                    break
                                overlap = chunk[-65536:]
                except tarfile.ReadError:
                    pass
result = {'image': image, 'files_checked': files, 'findings': findings}
pathlib.Path(report).write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({'image': image, 'files_checked': files, 'finding_count': len(findings)}))
if findings or files == 0:
    sys.exit('Image layer audit failed; see artifact paths')
