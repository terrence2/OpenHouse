# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp.dimension import Coord, Size

log = logging.getLogger('abode')


class _Property:
    def __init__(self, initial_value, configurable: bool=True):
        self.value_ = initial_value
        self.configurable_ = configurable

    @property
    def value(self):
        return self.value_

    @value.setter
    def value(self, new_value):
        if not self.configurable_:
            return
        self.value_ = new_value

    def is_configurable(self) -> bool:
        return self.configurable_


class Area:
    def __init__(self, parent, name: str, position: Coord, size: Size):
        # Intrinsic properties.
        self.parent = parent
        self.name = name
        self.position = position
        self.size = size

        # Sub-divisions of this space.
        self.subareas_ = {}  #: {Area.name: Area}

        # The set of properties attached to this region and the set of callbacks
        # directed at events on those properties.
        self.properties_ = {
            'size-x': _Property(self.size.x, False),
            'size-y': _Property(self.size.y, False),
            'size-z': _Property(self.size.z, False),
            'position-x': _Property(self.position.x, False),
            'position-y': _Property(self.position.y, False),
            'position-z': _Property(self.position.z, False),
        }  #: {str: _Property}
        self.listeners_ = {}  #: {str: {str: [callable]}} property -> event -> calls

    def subarea_names(self) -> [str]:
        """
        Return the names of all sub-areas in this area.
        """
        return list(self.subareas_.keys())

    def property_names(self) -> [str]:
        """
        Return the names of all properties on this area.
        """
        return list(self.properties_.keys())

    def path(self):
        """
        Construct and return the path that when passed to Abode.lookup will return
        this Area.
        """
        components = [self.name]
        parent = self.parent
        while parent is not None:
            components.append(parent.name)
            parent = parent.parent
        components = reversed(components)
        return '/' + '/'.join(components)

    def create_subarea(self, name: str, position: Coord, size: Size):
        """
        Instantiate and return a new Area that is a sub-area of the current area.
        """
        area = Area(self, name, position, size)
        self.subareas_[area.name] = area
        return area

    def subarea(self, name: str):
        """
        Return the named subarea.
        """
        return self.subareas_[name]

    def listen(self, prop_name: str, event_name: str, callback: callable):
        """
        Attach an event listener to call the given callback when the given
        event name occurs on the given property name.
        """
        if prop_name not in self.listeners_:
            self.listeners_[prop_name] = {}
        if event_name not in self.listeners_[prop_name]:
            self.listeners_[prop_name][event_name] = []
        self.listeners_[prop_name][event_name].append(callback)

    def set(self, prop_name: str, prop_value: object):
        """
        Update the value of a property on this area.
        """
        if prop_name not in self.properties_:
            log.info("ADD {}[{}]".format(self.name, prop_name))
            self.properties_[prop_name] = _Property(None)
            self.send_event(prop_name, 'propertyAdded', prop_value)
        if prop_value != self.properties_[prop_name].value:
            log.info("CHANGE {}[{}] = {} -> {}".format(self.name, prop_name, self.properties_[prop_name].value, prop_value))
            self.send_event(prop_name, 'propertyChanged', prop_value)
        log.debug("TOUCH {}[{}] = {}".format(self.name, prop_name, prop_value))
        self.properties_[prop_name].value = prop_value
        self.send_event(prop_name, 'propertyTouched', prop_value)

    def get(self, prop_name: str, default: object=None):
        """
        Get the value of a property.
        """
        if prop_name not in self.properties_:
            if default is not None:
                return default
            raise KeyError("Property {} not in {}".format(prop_name, self.path()))
        return self.properties_[prop_name].value

    def send_event(self, prop_name: str, event_name: str, prop_value: object=None):
        """
        Send an arbitrary event. An event must be fore a property name, but the property
        does not have to actually exist in the tree.
        """
        if prop_name not in self.listeners_:
            return
        if event_name not in self.listeners_[prop_name]:
            return
        event = AbodeEvent(event_name, self, prop_name, prop_value)
        for callback in self.listeners_[prop_name][event_name]:
            callback(event)


class AbodeEvent:
    def __init__(self, event_name: str, target: Area, prop_name: str, prop_value: object):
        self.event_name = event_name
        self.target = target
        self.property_name = prop_name
        self.property_value = prop_value


class Abode(Area):
    """
    A scene-graph specialize for houses. This is a graph of nested, inter-connected areas. The mapped
    areas are not "natural" in the physical sense, only what is convenient. Typically this will
    be rooms, areas within rooms, or logical subdivisions of a room, such as "dining room" and
    "kitchen", even when there is no separating wall.

    At the moment, without some sort of automatic mapping software like google's Tango, it is
    typically more convenient to specify an Abode in terms of axis aligned bounding rectangles,
    so this is what we do. Rooms' position is relative to Abode, Areas' position is relative to
    Room. There is no constraint that a subarea must be within the parent or not within some other
    peer in the graph. Whether this is convenient or dangerous depends on the use.

    Queries:
        Every Area has a name, position, and size. Areas can be found via a path using |lookup|,
        or found by name using |find|.

    Properties and Listeners:
        Arbitrary properties can be set on an area and queried later. Users of the Abode can
        register to receive notifications of property changes via |listen|. Current, changing
        a property triggers the following events:

        propertyAdded - The property was set for the first time. Not generally terribly useful
                        but included for completeness.
        propertyChanged - Triggered when the value of a property changes.
        propertyTouched - Triggered when the value of a property is set, but its value is the
                          same as the value that was there previously.
    """

    def __init__(self, name: str):
        super().__init__(None, name, Coord(0, 0), Size(0, 0, 0))

    room = Area.subarea
    create_room = Area.create_subarea

    def lookup(self, path: str):
        """
        Find an area by path.
        Of the form /<Abode.name>[/<Area.name>[/<Area.name>[/...]]]
        All components except prefix are optional.
        """
        assert path.startswith('/')
        assert len(path) > 1
        parts = path[1:].split('/')
        assert len(parts) > 0
        if parts[0] != self.name:
            raise KeyError("No Abode: " + parts[0])
        area = self
        parts = parts[1:]
        while parts:
            area = area.subarea(parts[0])
            parts = parts[1:]
        return area

