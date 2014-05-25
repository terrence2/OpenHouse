# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.abode import Abode, Area
from mcp.filesystem import FileSystem, File, Directory
from mcp.dimension import Coord, Size

house = \
    """
    Horizontal: 4/ft
    Vert: 2/ft
    Reference is upper-left corner.
    X is left-to-right.
    Y is top-to-bottom.
    Z is floor-to-ceiling.

    ----------------------------------------------------------------------------------------------------+
    |                                        |        .            *                                   |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |                                        |        .                                                |
    |         Office                         |        .                 Bedroom                        |
    |            10ftx13ft                   |________.                    12ftx10ft                   |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        .        |                                                |
    |                                        +________+@@@@@@@@@@--------------------------------------+
    |                                        @                         @                               |
    |                                        @        Hall             @                               |
    |                                        @          76" x 31"      @                               |
    |                                        @                         @                               |
    |@@@@@@-------------------------------------------+          +-----+                               |
    @                                                 @          @     |                               |
    @                                                 @          @     |                               |
    @  Entry                                          @          @     |                               |
    @    42" x 42"                                    @          @     |                               |
    @                                                 @          @     |                               |
    +--------------                                   +          +-----+-------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |     Living Room                                 |                                                |
    |        13' x 19'9"                              |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 +----------------@@@@@@@@@@@@---+@@@@@@@@@@------+
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    |                                                                                 |                |
    +-------------------------------------@@@@@@@@@@@@--------------------------------+----------------+
    """


def build_abode() -> Abode:
    abode = Abode("eyrie")
    office = abode.create_room('office', Coord(0, 0), Size('10ft', '13ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    livingroom = abode.create_room('livingroom', Coord(0, '13ft'), Size('13ft', '19ft9in', '8ft'))
    entry = livingroom.create_subarea('entry', Coord(0, 0), Size('42in', '42in', '8ft'))
    return abode


def bind_abode_to_filesystem(abode: Abode, fs: FileSystem):
    """
    Create a directory hierarchy that mirrors the abode layout.

    Note: it is generally most useful to do this after sensors and other inputs have
          bound themselves to the abode with properties.
    """
    def add_subareas(area: Area, area_dir: Directory):
        """Add sub-areas from the given area to the given directory. Recurse as needed."""
        for name in area.subarea_names():
            subarea_dir = area_dir.add_entry(name, Directory())
            subarea = area.subarea(name)
            add_subareas(subarea, subarea_dir)

        for property_name in area.property_names():
            def read_attr(bound_prop=property_name) -> str:
                return str(area.get(bound_prop)) + "\n"
            area_dir.add_entry(property_name, File(read_attr, None))

    abode_dir = fs.root().add_entry('abode', Directory())
    add_subareas(abode, abode_dir)


