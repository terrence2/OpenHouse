from actuators import HueBridge, HueLight
from floorplan import FloorPlan, Room, Alarm
from network import Network
from sensors import Nerve
from lib import Dim3, m

"""
Horizontal: 4/ft
Vert: 2/ft

-----------------------------------------------------------------------------------------------+
|                                        |    .            *                                   |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                                        |    .                                                |
|                         10ftx13ft      |____.                              12ftx10ft         |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        .    |                                                |
|                                        +____+@@@@@@@@@@--------------------------------------+
|                                        @                   @
|                                        @                   @
|                                        @                   @
|                                        @                   @
|@@@@@@----------------------------------+               +---+
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
@                                        @               @   |
+------+                                 +               +---+
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
|
+
"""


def build_floorplan() -> FloorPlan:

    fp = FloorPlan("Foothill")
    bedroom = fp.add_room(Room('Bedroom', Dim3('12ft', '10ft', '8ft')))
    bedroom.add_preset('off',
                       {'BedHue': {'on': False},
                        'DeskHue': {'on': False},
                        'DresserHue': {'on': False}})
    bedroom.add_preset('on',
                       {'BedHue': {'on': True, 'hsv': (255, 34495, 232)},
                        'DeskHue': {'on': True, 'hsv': (255, 34495, 232)},
                        'DresserHue': {'on': True, 'hsv': (255, 34495, 232)}})
    bedroom.add_preset('read',
                       {'BedHue': {'on': True, 'control': 'preset', 'hsv': (255, 34495, 232)},
                        'DeskHue': {'on': True, 'hsv': (0, 34495, 232)},
                        'DresserHue': {'on': True, 'hsv': (0, 34495, 232)}})
    bedroom.add_preset('sleep',
                       {'BedHue': {'on': False},
                        'DeskHue': {'on': True, 'hsv': (0, 47000, 255)},
                        'DresserHue': {'on': True, 'hsv': (0, 47000, 255)}})

    fp.add_alarm(Alarm('wakeup', hour=8, minute=0, second=0, callback=fp.do_sunrise))

    bridge = HueBridge('192.168.1.128', 'MasterControlProgram')
    fp.add_actuator(HueLight('BedHue', bridge, 1), [bedroom])
    fp.add_actuator(HueLight('DeskHue', bridge, 2), [bedroom])
    fp.add_actuator(HueLight('DresserHue', bridge, 3), [bedroom])

    bedroom_nerve = Nerve(fp, 'rpi-nerve-bedroom', ('rpi-nerve-bedroom', Network.DefaultSensorPort))
    fp.add_sensor(bedroom_nerve, 'Bedroom', (m('4ft'), 0), None)

    return fp
