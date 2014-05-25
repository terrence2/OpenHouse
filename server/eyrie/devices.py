# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from apscheduler.scheduler import Scheduler

import logging

from mcp.abode import Abode, Area
from mcp.color import BHS, RGB, Mired
from mcp.environment import Environment
from mcp.filesystem import FileSystem, Directory, File
from mcp.network import Bus as NetworkBus
from mcp.actuators.hue import HueBridge, HueLight
from mcp.devices import DeviceSet
from mcp.sensors.listener import Listener, ListenerEvent
from mcp.sensors.nerve import Nerve, NerveEvent

log = logging.getLogger("devices")


# Unfortunately, apscheduler doesn't really handle closures. Instead we have
# to communicate on the global: ugly and fragile.
# Dataflow Requirements:
#   * build_sensors must be called before apschduler is started.
#   * build_sensors must inject the two globals we require.
#   * build_sensors must add a job for this functions.
#   * When apscheduler starts, it will call this and expect the globals to be present.
environment_for_update_ = None
abode_for_update_ = None
def update_environment_on_abode():
    assert environment_for_update_ is not None
    assert abode_for_update_ is not None
    abode_for_update_.set('sunrise_twilight', environment_for_update_.sunrise_twilight)
    abode_for_update_.set('sunrise', environment_for_update_.sunrise)
    abode_for_update_.set('sunset', environment_for_update_.sunset)
    abode_for_update_.set('sunset_twilight', environment_for_update_.sunset_twilight)


def build_sensors(abode: Abode, environment: Environment, network: NetworkBus, scheduler: Scheduler) -> DeviceSet:
    sensors = DeviceSet()

    # Nerves.
    for unique in ('bedroom-north', 'office-north', 'livingroom-south'):
        name = 'nerve-{}'.format(unique)
        log.info("Building nerve: {}".format(name))
        nerve = sensors.add(Nerve(name, (name, NetworkBus.DefaultSensorPort)))
        path = '/eyrie/{}'.format(nerve.room_name)
        room = abode.lookup(path)

        # Put the properties on the abode with a default unset value so that we can know about them.
        room.set('temperature', 'unset')
        room.set('humidity', 'unset')
        room.set('motion', 'unset')

        # Forward updates to the sensor to the abode properties we just attached.
        def make_property_forwarder(bound_room: Area, bound_property_name: str):
            def handler(event: NerveEvent):
                log.info("{}[{}] = {}".format(bound_room.name, bound_property_name, event.value))
                bound_room.set(bound_property_name, event.value)
            return handler
        nerve.listen_temperature(make_property_forwarder(room, 'temperature'))
        nerve.listen_humidity(make_property_forwarder(room, 'humidity'))
        nerve.listen_motion(make_property_forwarder(room, 'motion'))

        # Put on the network.
        network.add_sensor(nerve.remote)

    # Listeners.
    abode.set('control', 'auto:daytime')
    for (room_name, machine) in [('bedroom', 'lemur')]:
        name = 'listener-{}-{}'.format(room_name, machine)
        log.info("Building listener: {}".format(name))
        listener = sensors.add(Listener(name, (machine, NetworkBus.DefaultSensorPort)))
        assert listener.room_name == room_name

        # Forward the commands to the control property.
        def property_forwarder(event: ListenerEvent):
            log.info("/eyrie[control] = {}".format(event.command))
            abode.set('control', event.command)
        listener.listen_for_commands(property_forwarder)

        # Put on the network.
        network.add_sensor(listener.remote)

    # Environment.
    # See comment above update_environment_on_abode for details on why this is insane.
    global abode_for_update_
    global environment_for_update_
    abode_for_update_ = abode
    environment_for_update_ = environment
    update_environment_on_abode()
    scheduler.add_interval_job(update_environment_on_abode, hours=12)

    return sensors


def build_actuators(filesystem: FileSystem) -> DeviceSet:
    actuators = DeviceSet()

    # Hue Lights
    bridge = HueBridge('hue-bedroom', 'MasterControlProgram')
    actuators.add(HueLight('hue-bedroom-bed', bridge, 1))
    actuators.add(HueLight('hue-bedroom-desk', bridge, 6))
    actuators.add(HueLight('hue-bedroom-dresser', bridge, 7))
    actuators.add(HueLight('hue-bedroom-torch', bridge, 3))
    actuators.add(HueLight('hue-office-ceiling1', bridge, 4))
    actuators.add(HueLight('hue-office-ceiling2', bridge, 5))
    actuators.add(HueLight('hue-livingroom-torch', bridge, 2))

    return actuators


def _bind_hue_light_to_filesystem(parent: Directory, hue: HueLight):
    subdir = parent.add_entry(hue.name, Directory())

    def read_on() -> str:
        return str(hue.on) + "\n"
    def write_on(data: str):
        hue.on = data.strip() == "True"
    subdir.add_entry("on", File(read_on, write_on))

    def read_bhs() -> str:
        return "{}\n".format(str(hue.bhs))
    def write_bhs(data: str):
        try:
            parts = data.strip().split()
            parts = [int(p) for p in parts]
            hue.bhs = BHS(*parts)
        except Exception as e:
            log.exception("failed to write bhs data")
            return
    subdir.add_entry("bhs", File(read_bhs, write_bhs))

    def read_rgb() -> str:
        return "{}\n".format(str(hue.rgb))
    def write_rgb(data: str):
        data = data.strip()
        try:
            if data.startswith('#'):
                if len(data) == 4:
                    r = int(data[1], 16) * 16
                    g = int(data[2], 16) * 16
                    b = int(data[3], 16) * 16
                elif len(data) == 7:
                    r = int(data[1:3], 16)
                    g = int(data[3:5], 16)
                    b = int(data[5:7], 16)
                else:
                    raise AssertionError("HTML format must have 3 or 6 chars: "
                                         + str(len(data)) + ':' + data)
            else:
                r, g, b = [int(p) for p in data.strip().split()]
            hue.rgb = RGB(r, g, b)
        except Exception as e:
            log.exception("failed to write rgb data")
            return
    subdir.add_entry("rgb", File(read_rgb, write_rgb))

    def read_mired() -> str:
        return "{}\n".format(hue.mired)
    def write_mired(data: str):
        try:
            hue.mired = Mired(int(data))
        except Exception as e:
            log.exception("failed to write mired data")
            return
    subdir.add_entry("mired", File(read_mired, write_mired))


def bind_actuators_to_filesystem(actuators: DeviceSet, filesystem: FileSystem):
    directory = filesystem.root().add_entry("actuators", Directory())
    for light in actuators.select("$hue"):
        _bind_hue_light_to_filesystem(directory, light)

