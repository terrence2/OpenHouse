# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.abode import build_abode, bind_abode_to_filesystem, bind_abode_to_state
from eyrie.actuators import build_actuators, bind_actuators_to_filesystem
from eyrie.alarms import populate_alarms_and_bind_to_state, bind_alarms_to_filesystem
from eyrie.automatic import bind_abode_to_real_world_obeying_state
from eyrie.state import EyrieStateMachine, bind_state_to_filesystem
from eyrie.database import bind_abode_to_database
from eyrie.presets import bind_preset_states_to_real_world
from eyrie.sensors import build_sensors

from mcp.animation import AnimationController
from mcp.environment import Environment
from mcp.filesystem import FileSystem
from mcp.network import Bus as NetworkBus

from apscheduler.scheduler import Scheduler

import llfuse

import os
import os.path


class Eyrie:
    def __init__(self, db_path: str):
        # Pre-init tasks. This has to be done before apscheduler init, since it's database of saved
        # tasks requires the callee functions to be right on some module's global.
        self.state = EyrieStateMachine('manual:unset')
        populate_alarms_and_bind_to_state(self.state)

        # Platform services.
        self.scheduler = Scheduler({'apscheduler.jobstore.default.class': 'apscheduler.jobstores.shelve_store:ShelveJobStore',
                                    'apscheduler.jobstore.default.path': os.path.join(db_path, 'scheduled_jobs.db')})
        self.filesystem = FileSystem('/things')
        self.network = NetworkBus(llfuse.lock)
        self.environment = Environment()
        self.animator = AnimationController(0.5, llfuse.lock)

        # The model.
        self.abode = build_abode()
        self.sensors = build_sensors(self.abode, self.environment, self.network, self.scheduler)

        # The view.
        self.actuators = build_actuators()

        # Data-binding for monitoring and direct control.
        bind_abode_to_database(self.abode, db_path)
        bind_abode_to_filesystem(self.abode, self.filesystem)
        bind_actuators_to_filesystem(self.actuators, self.filesystem)
        bind_alarms_to_filesystem(self.scheduler, self.filesystem)
        bind_state_to_filesystem(self.state, self.filesystem)
        # Data-binding for direct control.
        bind_abode_to_state(self.abode, self.state)
        bind_preset_states_to_real_world(self.state, self.actuators)
        # Data-binding for automatic management of the world based directly on senor readings.
        bind_abode_to_real_world_obeying_state(self.abode, self.actuators, self.animator, self.state)

    def run(self):
        # Off-main-thread.
        self.network.start()
        self.scheduler.start()
        self.animator.start()

        # Block the main thread. Must be unmounted to stop.
        self.filesystem.run()

    def cleanup(self):
        # Stop and wait for the off-main-thread jobs.
        self.network.exit()
        self.animator.exit()

        self.network.join()
        self.animator.join()
        self.scheduler.shutdown()
