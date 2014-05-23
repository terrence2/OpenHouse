# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

from mcp.abode import Abode
from mcp.filesystem import FileSystem, Directory
from mcp.fs_reflector import add_hue_light
from mcp.network import Bus as NetworkBus
from mcp.actuators.hue import HueBridge, HueLight
from mcp.devices import DeviceSet
from mcp.sensors.listener import Listener, ListenerEvent
from mcp.sensors.nerve import Nerve, NerveEvent

#from eyrie_controller import EyrieController
EyrieController = None

log = logging.getLogger("devices")


def build_sensors(abode: Abode, network: NetworkBus, controller: EyrieController) -> DeviceSet:
    devices = DeviceSet()

    # Nerves
    for unique in ('bedroom-north', 'office-north', 'livingroom-south'):
        name = 'nerve-{}'.format(unique)
        log.info("Building nerve: {}".format(name))
        nerve = devices.add(Nerve(name, (name, NetworkBus.DefaultSensorPort)))
        path = '/eyrie/{}'.format(nerve.room_name)

        def property_forwarder(path_inner: str, prop_inner: str):
            def handler(evt: NerveEvent):
                log.info("{}[{}] = {}".format(path_inner, prop_inner, evt.value))
                abode.lookup(path_inner).set(prop_inner, evt.value)
            return handler

        nerve.listen_temperature(property_forwarder(path, 'temperature'))
        nerve.listen_humidity(property_forwarder(path, 'humidity'))
        nerve.listen_motion(property_forwarder(path, 'motion'))
        network.add_sensor(nerve.remote)

    # Listeners
    for (name, machine) in [('listener-bedroom-chimp', 'lemur')]:
        def command_forwarder(controller: EyrieController):
            def on_command(event: ListenerEvent):
                log.warning("Received command: {}".format(event.command))
                controller.apply_preset(event.command.lower(), "*")
            return on_command
        listener = Listener(name, (machine, NetworkBus.DefaultSensorPort))
        listener.listen_for_commands(command_forwarder(controller))
        network.add_sensor(listener.remote)
        devices.add(listener)

    return devices


def build_actuators(filesystem: FileSystem) -> DeviceSet:
    devices = DeviceSet()

    # Hue Lights
    bridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    devices.add(HueLight('hue-bedroom-bed', bridge, 1))
    devices.add(HueLight('hue-bedroom-desk', bridge, 6))
    devices.add(HueLight('hue-bedroom-dresser', bridge, 7))
    devices.add(HueLight('hue-bedroom-torch', bridge, 3))
    devices.add(HueLight('hue-office-ceiling1', bridge, 4))
    devices.add(HueLight('hue-office-ceiling2', bridge, 5))
    devices.add(HueLight('hue-livingroom-torch', bridge, 2))

    directory = filesystem.root().add_entry("actuators", Directory())
    for light in devices.select("$hue"):
        add_hue_light(directory, light)

    return devices

