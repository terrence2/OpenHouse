__author__ = 'terrence'

from mcp import network
from mcp.sensors import Sensor
from mcp.abode import Abode


class Nerve(Sensor):
    DatabaseLocation = "/storage/raid/data/var/db/mcp/{}.rrd"

    def __init__(self, abode: Abode, name: str, address: (str, int)):
        super().__init__()
        self.abode = abode
        self.name = name
        self.address = address
        self.remote = network.Sensor(self)

        self.database_filename = self.DatabaseLocation.format(self.name)

        self.last_temperature = None
        self.last_humidity = None
        self.last_motion_state = False

        self._fs_celsius = File(self.read_celsius, None)
        self._fs_fahrenheit = File(self.read_fahrenheit, None)
        self._fs_humidity = File(self.read_humidity, None)
        self._fs_motion = File(self.read_motion, None)

    def read_celsius(self) -> str:
        if self.last_temperature is None:
            return "Waiting for reading\n"
        return str(self.last_temperature) + "\n"

    def read_fahrenheit(self) -> str:
        if self.last_temperature is None:
            return "Waiting for reading\n"
        return str(self.last_temperature * 9.0 / 5.0 + 32.0) + "\n"

    def read_humidity(self) -> str:
        if self.last_humidity is None:
            return "Waiting for reading\n"
        return str(self.last_humidity) + "\n"

    def read_motion(self) -> str:
        return str(self.last_motion_state) + "\n"

    def handle_sensor_message(self, json):
        """
        Called by the sensor model to inform us of new messages from the
        network.
        """
        msg_type = json['type']
        if msg_type == 'TEMP_HUMIDITY':
            self.last_temperature = float(json['temp'])
            self.last_humidity = float(json['humidity'])
            log.debug("Nerve TEMP_HUMIDITY: {} {}".format(self.last_temperature, self.last_humidity))
            subprocess.check_output(["rrdtool", "update", self.database_filename, "--",
                                     "N:{}:{}".format(self.last_temperature, self.last_humidity)])
        elif msg_type == 'MOVEMENT':
            self.last_motion_state = bool(json['state'])
            log.debug("Movement state: {}".format(self.last_motion_state))
        else:
            log.error("Unrecognized message type from Nerve {}: {}".format(self.name, msg_type))


