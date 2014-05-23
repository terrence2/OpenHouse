# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from mcp.abode import Abode
from mcp.environment import Environment
from mcp.filesystem import FileSystem
from mcp.fs_reflector import map_abode_to_filesystem, add_ro_object_properties, add_rw_abode_properties
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


def build_abode(filesystem: FileSystem, environment: Environment):
    abode = Abode("eyrie")
    office = abode.create_room('office', Coord(0, 0), Size('10ft', '13ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('12ft', '10ft', '8ft'))
    livingroom = abode.create_room('livingroom', Coord(0, '13ft'), Size('13ft', '19ft9in', '8ft'))
    entry = livingroom.create_subarea('entry', Coord(0, 0), Size('42in', '42in', '8ft'))

    # Create a directory structure out of abode.
    directories = map_abode_to_filesystem(abode, filesystem)

    # Add sensor nodes to the filesystem -- we cannot auto-detect their presence, since these will only get put
    # in the abode when we start receiving sensor data.
    for area in (office, bedroom, livingroom):
        add_rw_abode_properties(directories[area], area, ('temperature', 'humidity', 'motion'))

    # Show our environment data on the top-level abode node.
    add_ro_object_properties(directories[abode], environment,
                             ('sunrise_twilight', 'sunrise', 'sunset', 'sunset_twilight'))

    return abode


