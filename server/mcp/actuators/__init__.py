# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp import Device

log = logging.getLogger('actuators')


class Actuator(Device):
    def on_reply(self, message: object):
        log.warning("ignoring reply message to device {}: {}".format(self.name, message))
