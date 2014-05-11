# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
__author__ = 'terrence'

import logging
from mcp.abode import Abode, Area
from mcp.filesystem import FileSystem, Directory, File
from mcp.actuators.hue import HueLight

log = logging.getLogger('fs-reflector')


def map_abode_to_filesystem(abode: Abode, fs: FileSystem) -> [Directory]:
    """
    Create a directory hierarchy that mirrors the abode layout.
    """
    directories = {}

    def add_subareas(area: Area, area_dir: Directory):
        "Add sub-areas from the given area to the given directory."
        nonlocal directories
        directories[area] = area_dir
        for name in area.subarea_names():
            subarea_dir = area_dir.add_entry(name, Directory())
            subarea = area.subarea(name)
            add_subareas(subarea, subarea_dir)

    abode_dir = fs.root().add_entry('abode', Directory())
    add_subareas(abode, abode_dir)
    return directories


def add_rw_abode_properties(directory: Directory, area: Area, properties: [str]):
    """Install the given properties on |directory| to pass through to get/set on |area|."""
    for prop in properties:
        def read_prop(bound_prop=prop) -> str:
            try:
                return str(area.get(bound_prop)) + "\n"
            except KeyError:
                log.error("key not found looking up property {} on area {}".format(bound_prop, area.name))
                return ""

        def write_prop(data: str, bound_prop=prop):
            area.set(bound_prop, data.strip())

        directory.add_entry(prop, File(read_prop, write_prop))


def add_ro_object_properties(directory: Directory, obj: object, properties: [str]):
    """Install the given properties on |directory| to pass through to getattr on |obj|."""
    for prop in properties:
        def read_attr(bound_prop=prop) -> str:
            return lambda: getattr(obj, bound_prop)
        directory.add_entry(prop, File(read_attr, None))


def add_hue_light(parent: Directory, hue: HueLight):
    subdir = parent.add_entry(hue.name, Directory())

    def read_on() -> str:
        return str(hue.on) + "\n"
    def write_on(data: str):
        hue.on = data.strip() == "True"
    subdir.add_entry("on", File(read_on, write_on))

    def read_hsv() -> str:
        return "Hue: {}, Saturation: {}, V: {}\n".format(*hue.hsv)
    def write_hsv(data: str):
        try:
            parts = data.strip().split()
            parts = [int(p) for p in parts]
            hue.hsv = parts
        except Exception as e:
            log.warn(str(e))
            return
    subdir.add_entry("hsv", File(read_hsv, write_hsv))

    def read_rgb() -> str:
        return "0x{:02X}{:02X}{:02X}".format(*hue.rgb)
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
            self.rgb = (r, g, b)
        except Exception as e:
            log.warn(str(e))
            return
    subdir.add_entry("rgb", File(read_rgb, write_rgb))

    """
    subdir.add_entry("colortemp")
    """


