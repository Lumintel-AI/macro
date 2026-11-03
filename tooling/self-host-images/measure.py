#!/usr/bin/env python3
"""Sample host memory and filesystem use, including Docker's build processes."""
import csv
import os
import pathlib
import shutil
import sys
import time

out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
with (out / 'resources.csv').open('w', buffering=1) as stream:
    writer = csv.writer(stream)
    writer.writerow(['epoch', 'memory_used_bytes', 'memory_available_bytes',
                     'disk_used_bytes', 'disk_free_bytes'])
    while not (out / 'stop').exists():
        mem = {key: int(value.split()[0]) * 1024 for key, value in
               (line.split(':', 1) for line in pathlib.Path('/proc/meminfo').read_text().splitlines())}
        disk = shutil.disk_usage(os.getcwd())
        writer.writerow([int(time.time()), mem['MemTotal'] - mem['MemAvailable'],
                         mem['MemAvailable'], disk.used, disk.free])
        time.sleep(5)
