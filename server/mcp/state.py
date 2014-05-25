# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import logging

log = logging.getLogger('state')


class State:
    Manual = 0

    Wakeup = 1
    Daytime = 2
    Bedtime = 3
    Sleep = 4

    @staticmethod
    def to_string(state: int) -> str:
        return ['manual', 'wakeup', 'daytime', 'bedtime', 'sleep'][state]

    @staticmethod
    def from_string(name: str) -> int:
        return {
            'manual': State.Manual,
            'wakeup': State.Wakeup,
            'daytime': State.Daytime,
            'bedtime': State.Bedtime,
            'sleep': State.Sleep,
        }[name]


class StateEvent:
    def __init__(self, prior: int, new: int):
        self.prior_state = prior
        self.new_state = new

    def __str__(self):
        return "StateEvent({} -> {})".format(State.to_string(self.prior_state),
                                             State.to_string(self.new_state))


class StateMachine:
    def __init__(self):
        self.state_ = State.Manual

        self.on_enter_manual_ = []  # [callable]
        self.on_leave_manual_ = []  # [callable]
        self.on_wakeup_ = []  # [callable]
        self.on_daytime_ = []  # [callable]
        self.on_bedtime_ = []  # [callable]
        self.on_sleep_ = []  # [callable]

    @property
    def current(self):
        return self.state_

    @staticmethod
    def _listen(name: str, callback: callable, collection: [callable]) -> bool:
        log.info("{}: registering callback.".format(name))
        if callback in collection:
            log.info("Skipping adding duplicate callback for {}.".format(name))
            return False
        collection.append(callback)
        return True

    def listen_enter_manual(self, callback: callable) -> bool:
        return self._listen("EnterManual", callback, self.on_enter_manual_)

    def listen_leave_manual(self, callback: callable) -> bool:
        return self._listen("LeaveManual", callback, self.on_leave_manual_)

    def listen_wakeup(self, callback: callable) -> bool:
        return self._listen("Wakeup", callback, self.on_wakeup_)

    def listen_daytime(self, callback: callable) -> bool:
        return self._listen("Daytime", callback, self.on_daytime_)

    def listen_bedtime(self, callback: callable) -> bool:
        return self._listen("Bedtime", callback, self.on_bedtime_)

    def listen_sleep(self, callback: callable) -> bool:
        return self._listen("Sleep", callback, self.on_sleep_)

    @staticmethod
    def _dispatch(callbacks: [callable], event: StateEvent) -> [bool]:
        log.debug("Dispatching event {} to {} listeners.".format(str(event), len(callbacks)))
        results = [callback(event) for callback in callbacks]
        return [bool(result) or result is None for result in results]

    def enter_manual(self):
        log.info("Manual mode requested")
        if self.state_ == State.Manual:
            log.info("SKIP Manual: already in state Manual.")
            return False
        prior = self.state_
        self.state_ = State.Manual
        return all(self._dispatch(self.on_enter_manual_, StateEvent(prior, State.Manual)))

    def leave_manual(self, state: int):
        log.info("Auto:{} mode requested".format(State.to_string(state)))
        if self.state_ != State.Manual:
            log.info("SKIP Leave Manual: not in state Manual.")
            return False
        if state == State.Manual:
            log.info("SKIP Leave Manual: already in manual mode.")
            return False
        self.state_ = state
        callbacks = {
            State.Wakeup: self.on_wakeup_,
            State.Daytime: self.on_daytime_,
            State.Bedtime: self.on_bedtime_,
            State.Sleep: self.on_sleep_
        }[state]
        event = StateEvent(State.Manual, state)
        return all(self._dispatch(self.on_leave_manual_, event) + self._dispatch(callbacks, event))

    def _transition(self, name: str, state: int, callbacks: [callable]) -> bool:
        log.info("{} requested".format(name))
        if self.state_ == State.Manual:
            log.info("SKIP {}: in manual control mode.".format(name))
            return False
        if self.state_ == state:
            log.info("SKIP {0}: already in state {0}.".format(name))
            return False
        prior = self.state_
        self.state_ = state
        return all(self._dispatch(callbacks, StateEvent(prior, state)))

    def wakeup(self):
        return self._transition("Wakeup", State.Wakeup, self.on_wakeup_)

    def daytime(self):
        return self._transition("Daytime", State.Daytime, self.on_daytime_)

    def bedtime(self):
        return self._transition("Bedtime", State.Bedtime, self.on_bedtime_)

    def sleep(self):
        return self._transition("Sleep", State.Sleep, self.on_sleep_)

    def enter_state(self, state: int) -> bool:
        """
        Enter a programmatically obtained state.
        """
        log.info("enter_state({}) requested".format(State.to_string(state)))

        # If currently in manual, we cannot automatically leave for another state.
        if self.state_ == State.Manual:
            log.info("Skipping state change because we are in manual right now.")
            return

        if state == State.Manual:
            return self.enter_manual()
        elif state == State.Wakeup:
            return self.wakeup()
        elif state == State.Daytime:
            return self.daytime()
        elif state == State.Bedtime:
            return self.bedtime()
        elif state == State.Sleep:
            return self.sleep()

        return False

    def enter_user_state(self, state: int) -> bool:
        """
        Like enter_state, but done at direct user request, so leaving manual is okay.
        """
        if self.state_ == State.Manual:
            return self.leave_manual(state)
        self.enter_state(state)

