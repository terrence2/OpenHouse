# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp.actuators import Actuator
from mcp.actuators.hue import HueBridge, HueLight
from mcp.color import BHS, RGB, Mired
from mcp.devices import DeviceSet
from mcp.filesystem import FileSystem, File, Directory
from mcp.network import Bus as NetworkBus
from mcp.actuators.wemo import WeMoActuatorBridge, WeMoSwitch

log = logging.getLogger("actuators")

DaylightHue = 34495
DaylightSat = 232
MoonlightHue = 47000
MoonlightSat = 255


def daylight(brightness: float) -> BHS:
    """Return a BHS for pleasant light at the given relative brightness."""
    assert brightness >= 0
    assert brightness <= 1
    return BHS(255 * brightness, DaylightHue, DaylightSat)


with_ambient = """
def daylight_with_ambient(cls):
    Return a BHS for pleasant light, dimming the light when it is light outside, unless it is overcast.
"""


def moonlight(brightness: float) -> BHS:
    """Return a BHS for pleasant light to sleep by."""
    return BHS(255 * brightness, MoonlightHue, MoonlightSat)


def build_actuators(network: NetworkBus) -> DeviceSet:
    actuators = DeviceSet()

    # Hue Lights
    hue_bridge = HueBridge('hue-bridge', 'MasterControlProgram')
    actuators.add(HueLight('hue-bedroom-bed', hue_bridge, 1))
    actuators.add(HueLight('hue-bedroom-desk', hue_bridge, 6))
    actuators.add(HueLight('hue-bedroom-dresser', hue_bridge, 7))
    actuators.add(HueLight('hue-bedroom-torch', hue_bridge, 3))
    actuators.add(HueLight('hue-office-ceiling1', hue_bridge, 4))
    actuators.add(HueLight('hue-office-ceiling2', hue_bridge, 5))
    actuators.add(HueLight('hue-livingroom-torch', hue_bridge, 2))

    # WeMo Switches
    wemo_bridge = WeMoActuatorBridge('wemo-bridge')
    wemo_switch = actuators.add(WeMoSwitch('wemoswitch-office-fountain', wemo_bridge))
    network.add_device(wemo_switch.remote)

    return actuators


def _is_truthy(data: str) -> bool:
    normalized = data.strip().lower()
    return normalized == 'true' or normalized == 'on' or normalized == '1'


def _make_file(entity: Actuator, property_name: str, parser: callable):
    def _read() -> str:
        return str(getattr(entity, property_name)) + '\n'

    def _write(data: str):
        try:
            new_value = parser(data)
        except Exception:
            log.exception("failed to write property {} to {}; value was {}".format(property_name, entity, data))
            return
        setattr(entity, property_name, new_value)

    return File(_read, _write)


def _bind_hue_light_to_filesystem(parent: Directory, hue: HueLight):
    subdir = parent.add_entry(hue.name, Directory())

    subdir.add_entry('on', _make_file(hue, 'on', _is_truthy))

    def _parse_bhs(data: str):
        parts = data.strip().split()
        parts = [int(p) for p in parts]
        return BHS(*parts)
    subdir.add_entry('bhs', _make_file(hue, 'bhs', _parse_bhs))

    def _parse_rgb(data: str):
        data = data.strip()
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
        return RGB(r, g, b)
    subdir.add_entry('rgb', _make_file(hue, 'rgb', _parse_rgb))

    def _parse_mired(data: str):
        return Mired(int(data))
    subdir.add_entry('mired', _make_file(hue, 'mired', _parse_mired))


def _bind_wemo_switch_to_filesystem(parent: Directory, switch: WeMoSwitch):
    subdir = parent.add_entry(switch.name, Directory())
    subdir.add_entry('on', _make_file(switch, 'on', _is_truthy))


def bind_actuators_to_filesystem(actuators: DeviceSet, filesystem: FileSystem):
    directory = filesystem.root().add_entry("actuators", Directory())
    for light in actuators.select("$hue"):
        _bind_hue_light_to_filesystem(directory, light)

    for switch in actuators.select("$wemoswitch"):
        _bind_wemo_switch_to_filesystem(directory, switch)

