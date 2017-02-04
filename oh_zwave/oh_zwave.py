#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from pathlib import PurePosixPath as Path
from oh_shared.args import make_parser
from oh_shared.db import Tree, make_connection
from oh_shared.log import enable_logging
import asyncio
import logging
import os
import signal
import struct
import subprocess
import sys

log = logging.getLogger('oh_zwave')

EventType = 1
ValueType = 2


# Handle SIGTERM in python. This forces the interpretter to unwind the stack
# allowing us to kill our child before exiting. I'm not sure why it does not
# normally.
def sigterm_handler(signal, frame):
    sys.exit(0)
signal.signal(signal.SIGTERM, sigterm_handler)


async def build_id_map(tree: Tree):
    by_id = {}
    mds = await tree.get_matching_files("/room/*/zwave-motiondetector/*/id")
    for path, device_id in mds.items():
        by_id[int(device_id)] = Path(path).parent / 'raw-value'
        log.info("Mapping ZWave id {} to {}".format(device_id, by_id[int(device_id)]))
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
    try:
        while True:
            bs = os.read(rfd, 4098)
            log.debug("oh_zwave read {} bytes: {}".format(len(bs), bs))

            while len(bs) > 0:
                assert len(bs) >= 3, "malformed message from oh_zwave daemon"
                msg_type = int(bs[0])
                bs = bs[1:]

                if msg_type == EventType:
                    assert len(bs) == 2, "unexpected event message length"
                    device_id, value = struct.unpack('bb', bs)
                    bs = b''

                    if device_id not in target_by_id:
                        log.warning("got zwave message from unconfigured device {}".format(device_id))
                        continue

                    target = str(target_by_id[device_id])
                    await tree.set_file(target, str(value))

                elif msg_type == ValueType:
                    assert len(bs) == 6, "unexpected value message length"
                    device_id, value_kind, value = struct.unpack('bbi', bs)


    except KeyboardInterrupt:
        log.info("Got keyboard interrupt")
        pass

    finally:
        log.info("Cleaning up zwave daemon")
        os.close(rfd)
        os.close(wfd)
        proc.terminate()
        proc.wait()


async def main():
    parser = make_parser('A gateway for accepting zwave events into OpenHouse.')
    args = parser.parse_args()

    enable_logging(args.log_target, args.log_level)

    tree = await make_connection(args)
    target_by_id = await build_id_map(tree)
    device = await tree.get_file("/global/zwave-local-controller/device")
    if not os.path.isfile("./src/oh_zwave.cpp") and os.path.isfile("./oh_zwave/src/oh_zwave.cpp"):
        os.chdir('oh_zwave')

    try:
        await watch_devices(device, tree, target_by_id)
    except Exception as ex:
        log.exception("unexpected exception in watch_devices", exc_info=ex)



if __name__ == '__main__':
    sys.exit(asyncio.get_event_loop().run_until_complete(main()))
