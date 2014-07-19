# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from eyrie.state import EyrieStateMachine

from mcp.abode import Abode, Area, AbodeEvent
from mcp.filesystem import FileSystem, File, Directory
from mcp.dimension import Coord, Size

log = logging.getLogger('eyrie-abode')
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
    |                 (+2ft for closet)      .        |                       (+2ft for closet)        |
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
    |                                        @          7' x 6'        @                               |
    |                                        @                         @                               |
    |@@@@@@-------------------------------------------+          +-----+                               |
    @                                                 @          @     |                               |
    @                                                 @          @     |                               |
    @  Entry                                          @          @     |                               |
    @    42" x 42"                                    @          @     |                               |
    @                                                 @          @     |                               |
    +--------------                                   +@@@@@@@@@@+-----+-------------------------------+
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |                                                |
    |                                                 |          Kitchen                               |
    |                                                 |             12'x8'                             |
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
    |                                                      Dining Room                |   Utility      |
    |                                                         7'6"x8'6"               |     4'6"x8'6"  |
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
    abode = Abode('eyrie')
    office = abode.create_room('office', Coord(0, 0), Size('12ft', '13ft', '8ft'))
    if True:
        office.create_subarea('closet', Coord('10ft', '5ft'), Size('2ft6in', '5ft', '8ft'))
    bedroom = abode.create_room('bedroom', Coord('13ft', 0), Size('14ft', '10ft', '8ft'))
    if True:
        bedroom.create_subarea('closet', Coord(0, '5ft'), Size('2ft6in', '5ft', '8ft'))
    livingroom = abode.create_room('livingroom', Coord(0, '13ft'), Size('13ft', '19ft9in', '8ft'))
    if True:
        livingroom.create_subarea('entry', Coord(0, 0), Size('42in', '42in', '8ft'))
    hall = abode.create_room('hall', Coord('10ft', '10ft'), Size('76in', '31in', '8ft'))
    if True:
        hall.create_subarea('closet', Coord('5ft6in', '31in'), Size('1ft', '3ft', '8ft'))
    kitchen = abode.create_room('kitchen', Coord('13ft', '16ft'), Size('8ft', '12ft', '8ft'))
    utility = abode.create_room('utility', Coord('20ft6in', '24ft'), Size('4ft6in', '8ft6in', '8ft'))
    diningroom = abode.create_room('diningroom', Coord('13ft', '24ft'), Size('7ft6in', '8ft6in', '8ft'))

    return abode


def bind_abode_to_filesystem(abode: Abode, filesystem: FileSystem):
    """
    Create a directory hierarchy that mirrors the abode layout.

    Note: it is generally most useful to do this after sensors and other inputs have
          bound themselves to the abode with properties.
    """
    writable_properties = {'user_control'}

    def add_subareas(area: Area, area_dir: Directory):
        """Add sub-areas from the given area to the given directory. Recurse as needed."""
        for name in area.subarea_names():
            subarea_dir = area_dir.add_subdir(name, Directory())
            subarea = area.subarea(name)
            add_subareas(subarea, subarea_dir)

        for property_name in area.property_names():
            def read_attr(bound_prop=property_name) -> str:
                return str(area.get(bound_prop)) + "\n"

            def write_attr(data: str, bound_prop=property_name):
                data = data.strip()
                area.set(bound_prop, data)

            if property_name in writable_properties:
                node = File(read_attr, write_attr)
            else:
                node = File(read_attr, None)
            area_dir.add_file(property_name, node)

    abode_dir = filesystem.root().add_subdir(abode.name, Directory())
    add_subareas(abode, abode_dir)


def bind_abode_to_state(abode: Abode, state: EyrieStateMachine):
    """
    Allow the user to indicate control preferences by poking /things/eyrie/user_control.

    The state machine is responsible for handling all the intricacies here, so we can
    just pass through with a bit of filtering for bad values. If the user input is not
    a valid state, it's just set on /eyrie[user_control] and not reflected in the state.
    """
    def state_changed(event: AbodeEvent):
        log.info("Requested new state: {} -> {}".format(state.current, event.property_value))
        try:
            if not state.change_user_state(event.property_value):
                log.warning("Failed to enter new state.")
        except AssertionError:
            log.exception("Invalid state specified.")

    abode.listen('user_control', 'propertyTouched', state_changed)
