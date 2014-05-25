# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.abode import build_abode, bind_abode_to_filesystem
from eyrie.actuators import build_actuators, bind_actuators_to_filesystem
from eyrie.alarms import populate_alarms
from eyrie.database import bind_abode_to_database
from eyrie.sensors import build_sensors\

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
        # Pre-init tasks.
        populate_alarms(self)

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

        # Data-binding and or controller.
        # TODO: Implement a controller state.py with StateMachine. Hook up abode events to update the state.
        # TODO: Then hook up state-changed events to poke the outputs.
        #bind_model_to_state()
        #bind_state_to_view()
        bind_abode_to_database(self.abode, db_path)
        bind_abode_to_filesystem(self.abode, self.filesystem)
        bind_actuators_to_filesystem(self.actuators, self.filesystem)

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

    def trigger_alarm(self, name: str, day: str):
        """Callback for aps scheduler wakeup/sleep alarms."""
        pass
