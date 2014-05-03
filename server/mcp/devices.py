# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

from mcp import Device


class DeviceSet:
    """
    JQuery for reality.
    Expected name format is:
        <devicetype>-<roomname>-<devicename>
    Supported selectors are:
        $devicetype
        @roomname
    """

    def __init__(self, devices: {Device}=None):
        self.devices_ = devices or set()

    def add(self, device: Device):
        """Add one device to the set."""
        self.devices_.add(device)
        return device

    def __iter__(self):
        for device in self.devices_:
            yield device

    def __len__(self):
        return len(self.devices_)

    def __sub__(self, other):
        return DeviceSet(self.devices_ - other.devices_)

    def __str__(self):
        return "{" + ", ".join(device.name for device in self.devices_) + "}"

    def select_type_(self, device_type: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.device_type == device_type})

    def select_room_(self, room_name: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.room_name == room_name})

    def select_name_(self, device_name: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.device_name == device_name})

    def select(self, match: str) -> {Device}:
        if not match:
            return DeviceSet()
        if match == "*":
            return self
        if match[0] == '$':
            return self.select_type_(match[1:])
        elif match[0] == '@':
            return self.select_room_(match[1:])
        elif match[0] == '#':
            return self.select_name_(match[1:])
        return DeviceSet()

    def set(self, prop_name: str, prop_value) -> {Device}:
        for device in self.devices_:
            setattr(device, prop_name, prop_value)
        return self