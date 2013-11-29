import colorsys
import errno
import http.client
import json
import logging
import sys

from filesystem import File, Dir

log = logging.getLogger('actuators')


class Actuator(Dir):
    """
    Interface to a thing in the world which can be controlled.
    """

    def __init__(self, name:str):
        super().__init__(None)

        # All actuators must have a name.
        self.name = name

    # Re-use the parent link as the floorplan link.
    @property
    def floorplan(self):
        return self.parent
    @floorplan.setter
    def floorplan(self, fp):
        self.parent = fp


class ZmqActuator(Actuator):
    """
    An actuator which is available over the network via ZMQ.
    """
    def __init__(self, name:str, address:(str, int)):
        super().__init__(name)

        # Currently unused, as we simply bcast on the servo socket and servos
        # subscribe to get updates.
        self.address = address

        # The socket we need to broadcast on to reach our servo.
        self.socket = None

    def set_socket(self, sock):
        self.socket = sock

    def send_message(self, json):
        self.socket.send_json(json)


class LightStrip(ZmqActuator):
    """
    A USB connected arduino with a lightstrip on it that is accessible
    indirectly through a computer running mcp/actuator/lightstrip.py to provide
    a ZMQ endpoint.
    """
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.state = 's'

    def turn_on_full(self):
        self.send_message({
            'name': self.name,
            'type': 'ON'
        })

    def turn_off_full(self):
        self.send_message({
            'name': self.name,
            'type': 'OFF'
        })

    def color(self, r:int, g:int, b:int, t:float=1.0):
        self.send_message({
            'name': self.name,
            'type': 'GENERIC',
            'r': r,
            'g': g,
            'b': b,
            't': t
        })

    def send_test_message(self):
        self.send_message({
            'name': self.name,
            'type': 'TEST'
        })


class HueBridge(Dir):
    """
    A Philips Hue bridge which provides access to individual Hue lights.

    Since lights are only accessible behind a bridge, this class makes common
    the bridge attributes shared by multiple HueLight instances. It should be
    constructed and passed to HueLights, but generally not used directly.
    """
    def __init__(self, address:str, username:str):
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

    def listdir(self):
        return ["address", "username"]


class HueLight(Actuator):
    """
    An individually controllable Philips Hue light.
    """
    def __init__(self, name:str, bridge:HueBridge, hue_light_id:int):
        super().__init__(name)
        self.hue_bridge = bridge
        self.hue_light_id = hue_light_id

        self._fs_on = File(self.read_on, self.write_on)
        self._fs_hsv = File(self.read_hsv, self.write_hsv)
        self._fs_rgb = File(self.read_rgb, self.write_rgb)
        self._fs_colortemp = File(self.read_colortemp, self.write_colortemp)
        self._fs_control = File(self.read_control, None)
        self.control_ = ''

    # ON
    @property
    def on(self) -> bool:
        data = self.hue_bridge.request("GET", "/")
        return self.state_from(data)['on']

    @on.setter
    def on(self, value:bool):
        self.hue_bridge.request("PUT", self.state_url(), {'on': bool(value)})

    def read_on(self) -> str:
        return str(self.on) + "\n"

    def write_on(self, data:str):
        self.on = data.startswith('true')

    # HSV
    @property
    def hsv(self) -> (int, int, int):
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return (state['bri'], state['hue'], state['sat'])

    @hsv.setter
    def hsv(self, value:(int, int, int)):
        self.hue_bridge.request("PUT", self.state_url(),
                            {'bri': value[0],
                             'hue': value[1],
                             'sat': value[2]})

    def read_hsv(self) -> str:
        return "{} {} {}\n".format(*self.hsv)

    def write_hsv(self, data:str):
        try:
            parts = data.strip().split()
            parts = [int(p) for p in parts]
            self.hsv = parts
        except Exception as e:
            log.warn(str(e))
            return

    # RGB
    @property
    def rgb(self) -> (int, int, int):
        data = self.hue_bridge.request("GET", "")
        state = self.state_from(data)
        return self.bhs_to_rgb((state['bri'], state['hue'], state['sat']))

    @rgb.setter
    def rgb(self, data:(int, int, int)):
        bri, hue, sat = self.rgb_to_bhs(data)
        self.hue_bridge.request("PUT", self.state_url(),
                                {'bri': bri,
                                 'hue': hue,
                                 'sat': sat})

    def read_rgb(self) -> str:
        return "{} {} {}\n".format(*self.rgb)

    def write_rgb(self, data:str):
        data = data.strip()
        try:
            if data.startswith('#'):
                if len(data) == 4:
                    r = int(data[1], 16) * 16
                    g = int(data[2], 16) * 16
                    b = int(data[3], 16) * 16
                elif len(data) == 7:
                    r = int(data[1:3], 16)
                    g = int(data[3:5], 16)
                    b = int(data[5:7], 16)
                else:
                    raise AssertionError("HTML format must have 3 or 6 chars: "
                                         + str(len(data)) + ':' + data)
            else:
                r, g, b = [int(p) for p in data.strip().split()]
            self.rgb = (r, g, b)
        except Exception as e:
            log.warn(str(e))
            return

    # Color Temperature
    @property
    def colortemp(self) -> int:
        "Mired color temperature."
        data = self.hue_bridge.request("GET", "")
        return int(self.state_from(data)['ct'])

    @colortemp.setter
    def colortemp(self, data:int):
        self.hue_bridge.request("PUT", self.state_url(), {'ct': data})

    def read_colortemp(self) -> str:
        return "{} in [153,500]".format(self.colortemp)

    def write_colortemp(self, data:str):
        self.colortemp = int(data.strip())

    # Action
    @property
    def control(self) -> str:
        """
        The current controling state of the light.

        When the control is empty, the light is allowed to be controlled
        by timers and events from the model. When the control is set,
        however, its value is the non-model action that set its state.
        """
        return self.control_

    @control.setter
    def control(self, value:str):
        self.control_ = value

    def read_control(self) -> str:
        return self.control_ + '\n'

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


