#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import functools
import logging
import shared.aiohome as home

from pprint import pformat

import shared.util as util

log = logging.getLogger("oh_databind")


class RoomState:
    def __init__(self, name: str):
        self.name = name
        self.switches_ = {}
        # TODO: self.motions_ = {}

    def are_humans_present(self):
        return any(self.switches_.values())

@asyncio.coroutine
def handle_switch_change(S, room_name: str, switch_path: str, switch_data: dict):
    log.info("switch state change: {}".format(switch_data))
    #S('room[name={}]'.format(room_name)).attr('humans_present', switch_data)

@asyncio.coroutine
def main():
    util.enable_logging('output.log', 'DEBUG')
    S = yield from home.connect(('localhost', 8080))

    room_states = {}
    rooms = yield from S('room').run()
    for room_path, room in rooms.items():
        room_name = room['attrs']['name']
        room_states[room_name] = RoomState(room_name)

        switches = yield from S('room[name={}] > wemo-switch'.format(room_name)).run()
        for switch_path, switch in switches.items():
            yield from S.subscribe(switch_path, functools.partial(handle_switch_change, S, room_name))




if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
