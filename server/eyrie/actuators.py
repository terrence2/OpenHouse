# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import errno
import logging

from threading import Lock

from mcp.actuators import Actuator
from mcp.actuators.hue import HueBridge, HueLight, HueLightGroup
from mcp.color import BHS, RGB, Mired
from mcp.devices import DeviceSet
from mcp.filesystem import FileSystem, File, Directory, StaticFile
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


def build_actuators(network: NetworkBus, gil: Lock) -> DeviceSet:
    actuators = DeviceSet()

    # Hue Lights
    hue_bridge = HueBridge('hue-bridge', 'MasterControlProgram', gil)
    actuators.add(HueLight('hue-bedroom-bed', hue_bridge))
    actuators.add(HueLight('hue-bedroom-desk', hue_bridge))
    actuators.add(HueLight('hue-bedroom-dresser', hue_bridge))
    actuators.add(HueLight('hue-bedroom-tree0', hue_bridge))
    actuators.add(HueLight('hue-bedroom-tree1', hue_bridge))
    actuators.add(HueLight('hue-bedroom-tree2', hue_bridge))
    actuators.add(HueLight('hue-bedroom-ceiling', hue_bridge))
    actuators.add(HueLight('hue-office-ceiling1', hue_bridge))
    actuators.add(HueLight('hue-office-ceiling2', hue_bridge))
    actuators.add(HueLight('hue-office-torch', hue_bridge))
    actuators.add(HueLight('hue-livingroom-torch', hue_bridge))
    actuators.add(HueLight('hue-utility-ceiling', hue_bridge))
    actuators.add(HueLight('hue-hall-ceiling0', hue_bridge))
    actuators.add(HueLight('hue-hall-ceiling1', hue_bridge))
    hue_bridge.start()

    hue_bridge.add_group(HueLightGroup(0, actuators.select('$hue')))

    # WeMo Switches
    # FIXME: install this somewhere permanent.
    wemo_bridge = WeMoActuatorBridge('127.0.0.1')
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
            log.exception("failed to parse property {} for {}; value was {}".format(property_name, entity.name, data))
            return errno.EINVAL
        args = {property_name: new_value}
        entity.set(**args)

    return File(_read, _write, fixed_size=4096)


def _bind_hue_light_to_filesystem(parent: Directory, hue: HueLight):
    subdir = parent.add_subdir(hue.name, Directory())

    def _parse_color(data: str):
        data = data.strip()

        if data.startswith('BHS('):
            return BHS(*[int(p.strip()) for p in data[4:].strip(')').split(',')])
        if data.startswith('RGB('):
            return RGB(*[int(p.strip()) for p in data[4:].strip(')').split(',')])
        elif data.startswith('Mired('):
            return Mired(int(data[6:].strip(')')))

        if not data.startswith('#'):
            log.warning("Attempted set color on {} with unrecognized format: {}".format(hue.name, data))
            raise SyntaxWarning

        if len(data) == 4:
            r = int(data[1], 16) * 16
            g = int(data[2], 16) * 16
            b = int(data[3], 16) * 16
        elif len(data) == 7:
            r = int(data[1:3], 16)
            g = int(data[3:5], 16)
            b = int(data[5:7], 16)
        else:
            log.warning("Attempted set color on {} with html format: {}".format(hue.name, data))
            raise SyntaxWarning
        return RGB(r, g, b)

    help_string = """Color is of the form:
    #RRGGBB
    RGB(r, g, b)
    HSB(h, s, b)
    Mired(ct)\n"""
    subdir.add_file('help', StaticFile(help_string))
    subdir.add_file('swversion', File(lambda: hue.swversion + "\n", None))
    subdir.add_file('modelid', File(lambda: hue.modelid + "\n", None))
    subdir.add_file('on', _make_file(hue, 'on', _is_truthy))
    subdir.add_file('color', _make_file(hue, 'color', _parse_color))



def _bind_wemo_switch_to_filesystem(parent: Directory, switch: WeMoSwitch):
    subdir = parent.add_subdir(switch.name, Directory())
    subdir.add_file('on', _make_file(switch, 'on', _is_truthy))


def bind_actuators_to_filesystem(actuators: DeviceSet, filesystem: FileSystem):
    directory = filesystem.root().add_subdir("actuators", Directory())
    for light in actuators.select("$hue"):
        _bind_hue_light_to_filesystem(directory, light)

    for switch in actuators.select("$wemoswitch"):
        _bind_wemo_switch_to_filesystem(directory, switch)

