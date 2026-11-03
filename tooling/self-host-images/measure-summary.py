#!/usr/bin/env python3
import csv
import json
import pathlib
import sys

out = pathlib.Path(sys.argv[1])
rows = list(csv.DictReader((out / 'resources.csv').open()))
if not rows:
    sys.exit('No resource samples collected')
summary = {
    'sample_interval_seconds': 5,
    'sampled_seconds': int(rows[-1]['epoch']) - int(rows[0]['epoch']),
    'baseline_memory_used_bytes': int(rows[0]['memory_used_bytes']),
    'peak_host_memory_used_bytes': max(int(r['memory_used_bytes']) for r in rows),
    'minimum_host_memory_available_bytes': min(int(r['memory_available_bytes']) for r in rows),
    'baseline_disk_used_bytes': int(rows[0]['disk_used_bytes']),
    'peak_disk_used_bytes': max(int(r['disk_used_bytes']) for r in rows),
    'minimum_disk_free_bytes': min(int(r['disk_free_bytes']) for r in rows),
}
summary['peak_disk_growth_bytes'] = summary['peak_disk_used_bytes'] - summary['baseline_disk_used_bytes']
(out / 'resources.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
