__author__ = 'terrence'

from mcp.abode import Abode
from mcp.filesystem import FileSystem, Directory, File
from mcp.actuators.hue import HueLight


def map_abode_to_filesystem(abode: Abode, fs: FileSystem):
    pass


def add_hue_light(parent: Directory, hue: HueLight):
    subdir = parent.add_entry(hue.name, Directory())

    def read_on() -> str:
        return str(hue.on) + "\n"
    def write_on(data: str):
        hue.on = data.strip() == "True"
    subdir.add_entry("on", File(read_on, write_on))
    """
    subdir.add_entry("hsv")
    subdir.add_entry("rgb")
    subdir.add_entry("colortemp")
    """


def map_devices_to_filesystem(devices: [], fs: FileSystem):
    act_dir = fs.root().add_entry("actuators", Directory())
    for device in devices:
        if isinstance(device, HueLight):
            add_hue_light(act_dir, device)
