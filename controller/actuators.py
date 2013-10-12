from network import Network

class Actuator:
    """
    Interface to a thing in the world which can be controlled.
    """

    def __init__(self, name:str):
        super().__init__()

        # All actuators must have a name.
        self.name = name

class ZmqActuator(Actuator):
    """
    An actuator which is available over the network via ZMQ.
    """
    def __init__(self, name:str, addr:(str, int)):
        super().__init__(name)

        # Currently unused, as we simply bcast on the servo socket and servos
        # subscribe to get updates.
        self.addr = addr

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

class HueBridge:
    """
    A Philips Hue bridge which provides access to individual Hue lights.

    Since lights are only accessible behind a bridge, this class makes common
    the bridge attributes shared by multiple HueLight instances. It should be
    constructed and passed to HueLights, but generally not used directly.
    """
    def __init__(self, address:str, username:str):
        self.address = addr
        self.username = username

    def request(self, mode, resource, data=None):
        conn = http.client.HTTPConnection(self.address)
        conn.request(mode, address, data)
        res = conn.getresponse()
        conn.close()
        return json.loads(str(result.read(), encoding='utf-8'))


class HueLight(Actuator):
    """
    An individually controllable Philips Hue light.
    """
    def __init__(self, name:str, bridge:HueBridge, id:int):
        super().__init__(name)
        self.bridge = bridge
        self.id = id

