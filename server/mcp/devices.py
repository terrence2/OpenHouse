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

    class DeviceRef:
        """
        Pre-sorts the device name for faster search later.
        """
        def __init__(self, device: Device):
            self.device = device
            self.device_type, self.room_name, self.device_name = device.name.split('-')
            assert all(part != 'unset' for part in device.name.split('-'))

    def __init__(self, devices: {Device}=None):
        self.devices_ = devices or set()

    def add(self, device: Device):
        """Add one device to the set."""
        self.devices_.add(self.DeviceRef(device))
        return device

    def __iter__(self):
        for ref in self.devices_:
            yield ref.device

    def __len__(self):
        return len(self.devices_)

    def __sub__(self, other):
        return DeviceSet(self.devices_ - other.devices_)

    def __str__(self):
        return "{" + ", ".join(d.device.name for d in self.devices_) + "}"

    def select_type_(self, device_type: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.device_type == device_type})

    def select_room_(self, room_name: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.room_name == room_name})

    def select_name_(self, device_name: str) -> {Device}:
        return DeviceSet({device for device in self.devices_ if device.device_name == device_name})

    def select(self, match: str) -> {Device}:
        if match[0] == '$':
            return self.select_type_(match[1:])
        elif match[0] == '@':
            return self.select_room_(match[1:])
        elif match[0] == '#':
            return self.select_name_(match[1:])
        return set()

    def set(self, prop_name: str, prop_value) -> {Device}:
        for ref in self.devices_:
            setattr(ref.device, prop_name, prop_value)
        return self