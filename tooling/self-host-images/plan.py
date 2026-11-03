#!/usr/bin/env python3
import json
import os
from pathlib import Path
rows = [line.split('\t') for line in Path('tooling/self-host-images/images.tsv').read_text().splitlines()]
groups = ['rust'] + [name for name, dockerfile in rows if dockerfile != 'docker/Dockerfile']
selected = os.environ['SELECTED_GROUP']
if selected != 'all':
    if selected not in groups:
        raise SystemExit('Unknown build group')
    groups = [selected]
print('matrix=' + json.dumps({'group': groups}, separators=(',', ':')))
