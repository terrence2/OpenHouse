# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.actuators import daylight, moonlight
from eyrie.state import EyrieStateMachine

from mcp.abode import Abode, AbodeEvent
from mcp.animation import AnimationController
from mcp.devices import DeviceSet
from mcp.state import StateEvent


def bind_abode_to_real_world_obeying_state(abode: Abode, actuators: DeviceSet, animation: AnimationController,
                                           state: EyrieStateMachine):
    # Disable any ongoing animation when we leave a mode. At the very least we'll want a different animation.
    def on_leave_state(_: StateEvent):
        animation.cancel_ongoing_animation()
    for name in state.all_states():
        state.listen_exit_state(name, on_leave_state)

    # Hook the motion detectors directly up to the light state. This is crude but good for now.
    def on_motion(event: AbodeEvent):
        if state.current != 'auto:daytime':
            return
        actuators.select('@' + event.target.name).set('on', True).set('bhs', daylight(1))

    abode.lookup("/eyrie/bedroom").listen("motion", "propertyChanged", on_motion)
    abode.lookup("/eyrie/livingroom").listen("motion", "propertyChanged", on_motion)
    abode.lookup("/eyrie/office").listen("motion", "propertyChanged", on_motion)
