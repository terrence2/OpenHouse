# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from eyrie.actuators import daylight, moonlight
from eyrie.state import EyrieStateMachine

from mcp.abode import Abode, AbodeEvent
from mcp.animation import AnimationController, LinearAnimation
from mcp.color import BHS
from mcp.devices import DeviceSet
from mcp.state import StateEvent

import logging

log = logging.getLogger('auto')

WakeupFadeTime = 30 * 60  # 30 min
SleepFadeTime = 30 * 60  # 30 min


def bind_abode_to_real_world_obeying_state(abode: Abode, actuators: DeviceSet, animation: AnimationController,
                                           state: EyrieStateMachine):
    # Disable any ongoing animation when we leave a state. At the very least we'll want a different animation.
    def on_leave_state(_: StateEvent):
        animation.cancel_ongoing_animation()
    for name in state.all_states():
        state.listen_exit_state(name, on_leave_state)

    def on_enter_daytime(_: StateEvent):
        actuators.select('$hue').set('bhs', daylight(1)).set('on', True)
    state.listen_enter_state('auto:daytime', on_enter_daytime)

    # TODO: This is quite crude, but should be useful for calibrating the hardware at least.
    # TODO: These need to either set a timeout, or tie into a kinect.
    # Hook the motion detectors directly up to the light state.
    def on_motion(event: AbodeEvent):
        if state.current != 'auto:daytime':
            log.debug("skipping motion update -- state is {}, not auto:daytime".format(state.current))
            return
        new_lighting = daylight(int(event.property_value))
        log.debug("motion updating lighting state in {} to {}".format(event.target.name, new_lighting))
        actuators.select('@' + event.target.name).set('bhs', new_lighting).set('on', True)
    abode.lookup("/eyrie/bedroom").listen("motion", "propertyChanged", on_motion)
    abode.lookup("/eyrie/livingroom").listen("motion", "propertyChanged", on_motion)
    abode.lookup("/eyrie/office").listen("motion", "propertyChanged", on_motion)

    # TODO: make these visually appealing. Just a fade in/out right now for simplicity.
    # Hook wakeup/bedtime to turn the lights on slowly.
    def on_enter_wakeup(evt: StateEvent):
        # Don't cycle us backwards if the user already pushed us forwards.
        if evt.prior_state == 'auto:daytime' and abode.get('user_control') == 'auto:daytime':
            state.change_user_state('auto:daytime')
            return

        # Move to the expected sleep state, but also turn on the bedside lamp.
        lights = actuators.select('$hue').select('@bedroom')
        lights.set('bhs', moonlight(0)).set('on', True)

        def tick(v: BHS):
            lights.set('bhs', v)

        def finish():
            state.change_state('auto:daytime')

        animation.animate(LinearAnimation(moonlight(0), daylight(1), WakeupFadeTime, tick, finish))
    state.listen_enter_state('auto:wakeup', on_enter_wakeup)

    def on_enter_bedtime(evt: StateEvent):
        # Don't cycle us backwards if the user already pushed us forwards.
        if evt.prior_state == 'auto:sleep' and abode.get('user_control') == 'auto:sleep':
            state.change_user_state('auto:sleep')
            return

        # The expected daytime state is less certain than the nighttime state, so just
        # fade from the initial state. In particular, leave the bedside lamp alone in case
        # we happen to be reading.
        lights = (actuators.select('$hue').select('@bedroom') - actuators.select('#bed'))

        def tick(v: BHS):
            lights.set('bhs', v)

        def finish():
            # This state change should shut off the bedside lamp.
            state.change_state('auto:sleep')

        # Note: we use the dresser lamp as a representative here. Really we should average or something.
        animation.animate(LinearAnimation(lights.select('#dresser').get('bhs'), moonlight(0), SleepFadeTime,
                                          tick, finish))
    state.listen_enter_state('auto:bedtime', on_enter_bedtime)

    def on_enter_sleep(_: StateEvent):
        actuators.select('$hue').set('bhs', moonlight(0))
        actuators.select('#bed').set('on', False)
    state.listen_enter_state('auto:sleep', on_enter_sleep)

