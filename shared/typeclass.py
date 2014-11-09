# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.


class DerivingEq:
    def __eq__(self, other):
        return isinstance(other, self.__class__) and self.__dict__ == other.__dict__

    def __ne__(self, other):
        return not self.__eq__(other)


class DerivingSub:
    """
    Requires that the class's __init__ args match all properties by name.
    """
    def __sub__(self, other):
        new_props = {}
        assert self.__dict__.keys() == other.__dict__.keys()
        for key in self.__dict__:
            new_props[key] = getattr(self, key) - getattr(other, key)
        return self.__class__(**new_props)


class DerivingAdd:
    """
    Requires that the class's __init__ args match all properties by name.
    """
    def __add__(self, other):
        new_props = {}
        assert self.__dict__.keys() == other.__dict__.keys()
        for key in self.__dict__:
            new_props[key] = getattr(self, key) + getattr(other, key)
        return self.__class__(**new_props)


class DerivingMul:
    """
    Requires that the class's __init__ args match all properties by name.
    """
    def __mul__(self, scale: float):
        new_props = {}
        for key in self.__dict__:
            new_props[key] = getattr(self, key) * scale
        return self.__class__(**new_props)
