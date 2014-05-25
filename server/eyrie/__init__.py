# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.abode import build_abode
from eyrie.alarms import populate_alarms
from eyrie.database import add_data_recorders
from eyrie.devices import build_sensors, build_actuators

from mcp.animation import AnimationController, Animation
from mcp.environment import Environment
from mcp.filesystem import FileSystem
from mcp.network import Bus as NetworkBus

from apscheduler.scheduler import Scheduler

import llfuse

import argparse
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
        # TODO: Rework build_abode to only create the abode. Then make build_sensors inject the property names.
        # TODO: Then we can walk the abode and reflect all properties to the fs without having to tell it what to create.
        # TODO: Make the listener update the abode, rather than having it poke the controller directly.
        self.abode = build_abode(self.filesystem, self.environment)
        self.sensors = build_sensors(self.abode, self.network, self)

        # TODO: Implement a controller state.py with StateMachine. Hook up abode events to update the state.
        # TODO: Then hook up state-changed events to poke the outputs.

        # The views.
        add_data_recorders(self.abode, db_path)
        self.actuators = build_actuators(self.filesystem)

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
