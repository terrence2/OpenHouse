# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp import Device


class Sensor(Device):
    pass


class SensorEvent:
    def __init__(self, type_):
        self.type = type_


class MotionEvent(SensorEvent):
    def __init__(self, value: bool):
        super().__init__('motion')
        self.value = value


class SwitchEvent(SensorEvent):
    def __init__(self, value: bool):
        super().__init__('switch')
        self.value = value


class TemperatureEvent(SensorEvent):
    def __init__(self, value: float):
        super().__init__('temperature')
        self.value = value


class HumidityEvent(SensorEvent):
    def __init__(self, value: float):
        super().__init__('humidity')
        self.value = value
