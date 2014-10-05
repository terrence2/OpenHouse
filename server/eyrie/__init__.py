# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.abode import build_abode, bind_abode_to_filesystem, bind_abode_to_state
from eyrie.actuators import build_actuators, bind_actuators_to_filesystem
from eyrie.alarms import bind_alarms_to_state, bind_alarms_to_filesystem
from eyrie.automatic import bind_abode_to_real_world_obeying_state
from eyrie.database import bind_abode_to_database
from eyrie.presence import bind_abode_to_presence
from eyrie.presets import bind_preset_states_to_real_world
from eyrie.sensors import build_sensors
from eyrie.state import EyrieStateMachine, bind_state_to_filesystem

from mcp.animation import AnimationController
from mcp.cronish import Cronish
from mcp.environment import Environment
from mcp.filesystem import FileSystem
from mcp.home import Home
from mcp.network import Bus as NetworkBus
from mcp.scheduler import Scheduler

import llfuse


class Eyrie:
    def __init__(self, db_path: str):
        # Platform services.
        self.animator = AnimationController(2, llfuse.lock)
        self.cronish = Cronish(db_path, llfuse.lock)
        self.environment = Environment()
        self.filesystem = FileSystem('/things')
        self.network = NetworkBus(llfuse.lock)
        self.scheduler = Scheduler(llfuse.lock)

        # The model.
        self.home = Home(llfuse.lock)
        self.abode = build_abode()
        self.sensors, sensor_threads = build_sensors(self.abode, self.home, self.environment, self.network, self.cronish,
                                                     self.scheduler)
        self.state = EyrieStateMachine('manual:unset')

        # The view.
        self.actuators = build_actuators(self.network, llfuse.lock)

        # Data-binding for monitoring and direct control.
        bind_abode_to_database(self.abode, db_path)
        bind_abode_to_presence(self.abode, self.cronish)
        bind_abode_to_filesystem(self.abode, self.filesystem)
        bind_actuators_to_filesystem(self.actuators, self.filesystem)
        bind_alarms_to_filesystem(self.cronish, self.filesystem)
        bind_state_to_filesystem(self.state, self.filesystem)
        # Data-binding for direct control.
        bind_abode_to_state(self.abode, self.state)
        bind_alarms_to_state(self.cronish, self.state)
        bind_preset_states_to_real_world(self.state, self.actuators)
        # Data-binding for automatic management of the world based directly on senor readings.
        bind_abode_to_real_world_obeying_state(self.abode, self.actuators, self.animator, self.state)

        self.threads = [
            self.animator,
            self.cronish,
            self.home,
            self.network,
            self.scheduler
        ] + sensor_threads

    def run(self):
        for thread in self.threads:
            thread.start()

        # Block the main thread. Must be unmounted to stop.
        self.filesystem.run()

    def cleanup(self):
        for thread in self.threads:
            thread.exit()

        for thread in self.threads:
            thread.join()
