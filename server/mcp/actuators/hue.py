# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import http
import http.client
import json
from mcp.actuators import Actuator


class HueBridge:
    """
    A Philips Hue bridge which provides access to individual Hue lights.

    Since lights are only accessible behind a bridge, this class makes common
    the bridge attributes shared by multiple HueLight instances. It should be
    constructed and passed to HueLights, but generally not used directly.
    """
    def __init__(self, address: str, username: str):
        super().__init__()
        self.address = address
        self.username = username

    def request(self, mode, resource, data=None):
        if data is not None:
            data = json.dumps(data).encode('UTF-8')
        conn = http.client.HTTPConnection(self.address)
        conn.request(mode, '/api/' + self.username + resource, data)
        res = conn.getresponse()
        data = res.read()
        conn.close()
        return json.loads(str(data, encoding='UTF-8'))


class HueLight(Actuator):
    """
    An individually controllable Philips Hue light.
    """
    def __init__(self, name:str, bridge:HueBridge, hue_light_id:int):
        super().__init__(name)
        self.hue_bridge = bridge
        self.hue_light_id = hue_light_id

        """
        self._fs_colortemp = File(self.read_colortemp, self.write_colortemp)
        """

    # ON
    @property
    def on(self) -> bool:
        data = self.hue_bridge.request("GET", "/")
        return self.state_from(data)['on']

    @on.setter
    def on(self, value: bool):
        self.hue_bridge.request("PUT", self.state_url(), {'on': bool(value)})

    # HSV
    @property
    def hsv(self) -> (int, int, int):
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return (state['bri'], state['hue'], state['sat'])

    @hsv.setter
    def hsv(self, value: (int, int, int)):
        self.hue_bridge.request("PUT", self.state_url(),
                            {'bri': value[0],
                             'hue': value[1],
                             'sat': value[2]})

    # RGB
    @property
    def rgb(self) -> (int, int, int):
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return self.bhs_to_rgb((state['bri'], state['hue'], state['sat']))

    @rgb.setter
    def rgb(self, data: (int, int, int)):
        bri, hue, sat = self.rgb_to_bhs(data)
        self.hue_bridge.request("PUT", self.state_url(),
                                {'bri': bri,
                                 'hue': hue,
                                 'sat': sat})

    # Color Temperature
    @property
    def colortemp(self) -> int:
        """Mired color temperature."""
        data = self.hue_bridge.request("GET", "")
        return int(self.state_from(data)['ct'])

    @colortemp.setter
    def colortemp(self, data: int):
        self.hue_bridge.request("PUT", self.state_url(), {'ct': data})

    def read_colortemp(self) -> str:
        return "{} in [153,500]".format(self.colortemp)

    def write_colortemp(self, data: str):
        self.colortemp = int(data.strip())

    # Utility
    def state_url(self):
        return "/lights/{}/state".format(self.hue_light_id)

    def state_from(self, data):
        return data['lights'][str(self.hue_light_id)]['state']

    def rgb_to_bhs(self, rgb):
        r, g, b = [p / 256 for p in rgb]
        hue, light, sat = colorsys.rgb_to_hls(r, g, b)
        return (int(light * 256), int(hue * 2**16), int(sat * 256))

    def bhs_to_rgb(self, bhs):
        bri, hue, sat = [p / 256 for p in bhs]
        hue /= 256
        r, g, b = colorsys.hls_to_rgb(hue, bri, sat)
        return [int(p * 256) for p in (r, g, b)]



