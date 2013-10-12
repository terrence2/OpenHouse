from floorplan import FloorPlan
from actuators import HueBridge, HueLight

def build_floorplan():

    fp = FloorPlan("Foothill")

    bridge = HueBridge('192.168.1.128', 'MasterControlProgram')
    fp.add_actuator(HueLight('BedroomHueLightBed', bridge, 1)
    fp.add_actuator(HueLight('BedroomHueLightDesk', bridge, 2)
    fp.add_actuator(HueLight('BedroomHueLightDresser', bridge, 3)
