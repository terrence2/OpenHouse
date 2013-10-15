from floorplan import FloorPlan
from actuators import HueBridge, HueLight

def build_floorplan():

    fp = FloorPlan("Foothill")
    fp.add_room('Bedroom', 0, 0, 0)

    bridge = HueBridge('192.168.1.128', 'MasterControlProgram')
    fp.add_actuator(HueLight('BedroomHueLightBed', bridge, 1), 'Bedroom')
    fp.add_actuator(HueLight('BedroomHueLightDesk', bridge, 2), 'Bedroom')
    fp.add_actuator(HueLight('BedroomHueLightDresser', bridge, 3), 'Bedroom')

    return fp
