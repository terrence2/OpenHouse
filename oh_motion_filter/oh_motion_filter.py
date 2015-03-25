#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging

from oh_shared.home import Home, NodeData
from oh_shared.args import parse_default_args
from oh_shared.log import enable_logging


log = logging.getLogger('oh_motion_filter')


class MotionDetector:
    def __init__(self, name: str, home: Home):
        self.home = home
        self.log = logging.getLogger('oh_motion_filter.' + name)
        self.raw_state_ = False
        self.disable_handle_ = None

    @classmethod
    @asyncio.coroutine
    def create(cls, home: Home, path: str, node: NodeData) -> 'MotionDetector':
        motion = cls(node.name, home)
        motion.log.debug("watching {}".format(path))
        yield from home.subscribe(path, motion.on_state_change)
        return motion

    @asyncio.coroutine
    def on_state_change(self, path: str, node: NodeData):
        prior_raw_state = self.raw_state_
        next_raw_state = node.attrs.get('raw-state', 'false') == "true"
        current_state = node.attrs.get('state', 'false') == "true"

        # Update cached state for next event.
        self.raw_state_ = next_raw_state

        # False -> True
        if not prior_raw_state and next_raw_state:
            if current_state:
                assert self.disable_handle_ is not None
                self.log.debug("{} raw-state false->true while state is true; cancelling".format(path, self.raw_state_))
                self.disable_handle_.cancel()
                self.disable_handle_ = None
            else:
                self.log.debug("{} raw-state false->true while state is false; turning on".format(path))
                asyncio.async(self.set_true(path))
        # True -> False
        elif prior_raw_state and not next_raw_state:
            assert current_state
            delay = 500  # sec
            self.log.debug("{} raw-state true->false; turning off in {} seconds".format(path, delay))
            self.disable_handle_ = asyncio.async(self.set_false(delay, path))


    @asyncio.coroutine
    def set_false(self, delay: float or int, path: str):
        self.log.debug("waiting {} sec before setting {}[state=false]".format(delay, path))
        yield from asyncio.sleep(delay)
        self.log.info("{}[state=false]".format(path))
        yield from self.home.query_path(path).attr('state', False).run()

    @asyncio.coroutine
    def set_true(self, path: str):
        self.log.info("{}[state=true]".format(path))
        yield from self.home.query_path(path).attr('state', True).run()


@asyncio.coroutine
def main():
    args = parse_default_args('Filter raw motion states to get a more coherent and stable output.')
    enable_logging(args.log_target, args.log_level)
    home = yield from Home.connect((args.home_address, args.home_port))

    motion_detectors = []
    nodes = yield from home.query("motion").run()
    for path, node in nodes.items():
        motion = yield from MotionDetector.create(home, path, node)
        motion_detectors.append(motion)


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
