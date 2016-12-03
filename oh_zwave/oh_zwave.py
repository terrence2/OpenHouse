#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from aiohttp import web
from pathlib import PurePosixPath as Path
from oh_shared.args import make_parser
from oh_shared.db import Tree, TreeError, make_connection
from oh_shared.log import enable_logging
import asyncio
import logging
import os
import socket
import subprocess
import sys

log = logging.getLogger('oh_button')


async def build_id_map(tree: Tree):
    by_id = {}
    mds = await tree.get_matching_files("/room/*/zwave-motion-detector/*/id")
    for path, device_id in mds.items():
        by_id[int(device_id)] = Path(path).parent / 'value'
    return by_id


async def watch_devices(device: str, tree: Tree, target_by_id: {int: Path}):
    assert os.path.isfile("./src/oh_zwave.cpp")
    subprocess.run(["make"])
    rfd, wfd = os.pipe()
    proc = subprocess.Popen(["build/oh_zwave",
                             "--device", device,
                             "--event-fd", str(wfd)],
                            pass_fds=[wfd],
                            env={'LD_LIBRARY_PATH': '/usr/local/lib64'})
    while True:
        bs = os.read(rfd, 2)
        device_id = int(bs[0])
        value = int(bs[1])

        if device_id in target_by_id:
            target = str(target_by_id[device_id])
            await tree.set_file(target, str(value))


async def main():
    parser = make_parser('A gateway for accepting zwave events into OpenHouse.')
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    tree = await make_connection(args)
    target_by_id = await build_id_map(tree)
    device = await tree.get_file("/global/zwave-local-controller/device")
    if not os.path.isfile("./src/oh_zwave.cpp") and os.path.isfile("./oh_zwave/src/oh_zwave.cpp"):
        os.chdir('oh_zwave')
    await watch_devices(device, tree, target_by_id)


if __name__ == '__main__':
    sys.exit(asyncio.get_event_loop().run_until_complete(main()))
