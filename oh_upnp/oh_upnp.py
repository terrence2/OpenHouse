#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
"""
Maintains the UPnP table.
"""
import logging
import time

from threading import Lock

import util

from home import Home
from upnp import UPnP


log = logging.getLogger('oh_hue')


def setup_runtime(home: Home):
    res = home.query("[kind='runtime']").run()
    assert res, "No runtime node found on HOM."

    group = home.group()
    group.add_or_empty("[kind='runtime']", "[kind='runtime'] > [kind='upnp']", '<div kind="upnp" name="upnp"></div>')
    group.run()


def found_device(home: Home, address: (str, int), headers: {str: str}):
    node_query = "[kind='upnp'] > [kind='network-device'][ipv4='{}'][port='{}']".format(*address)
    node_content = '<div kind="network-device" name="{}" ipv4="{}" port="{}"></div>'.format(address[0], *address)
    group = home.group()
    group.add_or_empty("[kind='upnp']", node_query, node_content)
    group.reflect_as_properties(node_query, headers)
    group.run()


def main():
    util.enable_logging('events.log', 'DEBUG')
    gil = Lock()

    home = Home(gil)

    def _found_device(address, headers):
        try:
            found_device(home, address, headers)
        except Exception as e:
            log.exception(e)
    upnp = UPnP((util.get_own_internal_ip_slow(), 54323), _found_device, 60, gil)

    setup_runtime(home)

    upnp.start()

    while True:
        try:
            time.sleep(60 * 60 * 24)
        except KeyboardInterrupt:
            break

    upnp.exit()
    upnp.join()


if __name__ == '__main__':
    main()
