# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import colorsys
import http
import http.client
import json
from mcp.actuators import Actuator
from mcp.color import BHS, RGB, Mired


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

        self.known_lights = {}  # {name:str => id:str}
        lights = self.request('GET', '/lights')
        for id_str, light_def in lights.items():
            self.known_lights[light_def['name']] = id_str

    def identify_light(self, name:str):
        return self.known_lights[name]

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
    def __init__(self, name: str, bridge: HueBridge):
        super().__init__(name)
        self.hue_bridge = bridge
        self.hue_light_id = bridge.identify_light(name)

    # ON
    @property
    def on(self) -> bool:
        data = self.hue_bridge.request("GET", "/")
        return self.state_from(data)['on']

    @on.setter
    def on(self, value: bool):
        self.hue_bridge.request("PUT", self.state_url(), {'on': bool(value)})

    # BHS
    @property
    def bhs(self) -> BHS:
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return BHS(state['bri'], state['hue'], state['sat'])

    @bhs.setter
    def bhs(self, value: BHS):
        self.hue_bridge.request("PUT", self.state_url(),
                                {'bri': value.b,
                                 'hue': value.h,
                                 'sat': value.s})

    # RGB
    @property
    def rgb(self) -> RGB:
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return self.bhs_to_rgb(BHS(state['bri'], state['hue'], state['sat']))

    @rgb.setter
    def rgb(self, data: RGB):
        bhs = self.rgb_to_bhs(data)
        self.hue_bridge.request("PUT", self.state_url(),
                                {'bri': bhs.b,
                                 'hue': bhs.h,
                                 'sat': bhs.s})

    # Color Temperature
    @property
    def mired(self) -> Mired:
        """Mired color temperature."""
        data = self.hue_bridge.request("GET", "")
        return Mired(self.state_from(data)['ct'])

    @mired.setter
    def mired(self, data: Mired):
        self.hue_bridge.request("PUT", self.state_url(), {'ct': data.ct})

    # Utility
    def state_url(self):
        return "/lights/{}/state".format(self.hue_light_id)

    def state_from(self, data):
        return data['lights'][self.hue_light_id]['state']

    def rgb_to_bhs(self, rgb: RGB) -> BHS:
        r = rgb.r / 256
        g = rgb.g / 256
        b = rgb.b / 256
        hue, light, sat = colorsys.rgb_to_hls(r, g, b)
        return BHS(light * 256, hue * 2**16, sat * 256)

    def bhs_to_rgb(self, bhs: BHS) -> RGB:
        bri = bhs.b / 256
        hue = bhs.h / (2**16)
        sat = bhs.s / 256
        r, g, b = colorsys.hls_to_rgb(hue, bri, sat)
        return RGB(r * 256, g * 256, b * 256)



