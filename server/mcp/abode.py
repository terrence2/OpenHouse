__author__ = 'terrence'

from mcp.dimension import Coord, Size


class Area:
    def __init__(self, parent, name: str, position: Coord, size: Size):
        self.parent_ = parent

        self.name = name
        self.position = position
        self.size = size

        self.subareas_ = {}  #: {Area.name: Area}

        self.properties_ = {}  #: {str: any}

        self.listeners_ = {}  #: {str: [callable]}

    def create_subarea(self, name: str, position: Coord, size: Size):
        area = Area(self, name, position, size)
        self.subareas_[area.name] = area
        return area

    def subarea(self, name: str):
        return self.subareas_[name]

    def set(self, key: str, val: object):
        if key not in self.properties_:
            self.send_event('newProp', key, val)
        elif val != self.properties_[key]:
            self.send_event('propChanged', key, val)
        self.properties_[key] = val
        self.send_event('propUpdated', key)

    def send_event(self, name: str, key: str, **kwargs):
        if key not in
        event = Event(.name, self, **kwargs)



class Event:
    def __init__(self, name: str, target: Area, **kwargs):
        self.name = name
        self.target = target
        for k, v in kwargs.items():
            setattr(self, k, v)


class Abode(Area):
    """
    A scene-graph specialize for houses. This is a graph of nested, inter-connected areas. The mapped
    areas are not "natural" in the physical sense, only what is convenient. Typically this will
    be rooms, areas within rooms, or logical subdivisions of a room, such as "dining room" and
    "kitchen", even when there is no separating wall.

    At the moment, without some sort of automatic mapping software like google's Tango, it is
    typically more convenient to specify an Abode in terms of axis aligned bounding rectangles,
    so this is what we do. Rooms' position is relative to Abode, Areas' position is relative to
    Room.
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


