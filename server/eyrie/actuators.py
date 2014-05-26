# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp.actuators.hue import HueBridge, HueLight
from mcp.color import BHS, RGB, Mired
from mcp.devices import DeviceSet
from mcp.filesystem import FileSystem, File, Directory

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


def build_actuators() -> DeviceSet:
    actuators = DeviceSet()

    # Hue Lights
    bridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    actuators.add(HueLight('hue-bedroom-bed', bridge, 1))
    actuators.add(HueLight('hue-bedroom-desk', bridge, 6))
    actuators.add(HueLight('hue-bedroom-dresser', bridge, 7))
    actuators.add(HueLight('hue-bedroom-torch', bridge, 3))
    actuators.add(HueLight('hue-office-ceiling1', bridge, 4))
    actuators.add(HueLight('hue-office-ceiling2', bridge, 5))
    actuators.add(HueLight('hue-livingroom-torch', bridge, 2))

    return actuators


def _bind_hue_light_to_filesystem(parent: Directory, hue: HueLight):
    subdir = parent.add_entry(hue.name, Directory())

    def read_on() -> str:
        return str(hue.on) + "\n"
    def write_on(data: str):
        hue.on = data.strip() == "True"
    subdir.add_entry("on", File(read_on, write_on))

    def read_bhs() -> str:
        return "{}\n".format(str(hue.bhs))
    def write_bhs(data: str):
        try:
            parts = data.strip().split()
            parts = [int(p) for p in parts]
            hue.bhs = BHS(*parts)
        except Exception:
            log.exception("failed to write bhs data")
            return
    subdir.add_entry("bhs", File(read_bhs, write_bhs))

    def read_rgb() -> str:
        return "{}\n".format(str(hue.rgb))
    def write_rgb(data: str):
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
            hue.rgb = RGB(r, g, b)
        except Exception:
            log.exception("failed to write rgb data")
            return
    subdir.add_entry("rgb", File(read_rgb, write_rgb))

    def read_mired() -> str:
        return "{}\n".format(hue.mired)
    def write_mired(data: str):
        try:
            hue.mired = Mired(int(data))
        except Exception:
            log.exception("failed to write mired data")
            return
    subdir.add_entry("mired", File(read_mired, write_mired))


def bind_actuators_to_filesystem(actuators: DeviceSet, filesystem: FileSystem):
    directory = filesystem.root().add_entry("actuators", Directory())
    for light in actuators.select("$hue"):
        _bind_hue_light_to_filesystem(directory, light)