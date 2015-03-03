#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
"""
Use switch and motion states to infer human presence and activity.

A switch is configured as:
    <switch kind="<brand>" name="<identifier>"></switch>

  Required attributes:
    name - all addressable nodes must have a name.

    kind - a brand identifier that tells some other kind-specific bridging daemon what sort of hardware this is so
           that the hardware state can be tied to the state attribute.

  Optional attributes:
    context - by default the containing room. If a context is set, the switch's state will apply to the
              node(s) that result from using the context value as a query string, instead of the containing
              room. This is most commonly used to make a switch control multiple rooms: e.g.

              <switch kind="wemo" name="bar-plug" context="room[name=kitchen], room[name=diningroom]"></switch>

    activity - by default a switch being state="true" implies that humans are present, but nothing about what they are
               doing. Thus, the default is simply: "yes". If the switch should indicate some other activity,
               like "watching-a-movie", this can be configured by setting the "activity" attribute to this specific
               activity. The activity set on the context nodes can then be used by scenes to adjust the lighting.

  Runtime attributes:
    state - The brand-specific bridge daemon will set the 'state' attribute to some identifier based on the hardware.
            For on/off switches, motion detectors, and other binary switches, this is usually 'true' and 'false'.
            This daemon -- oh_infer_activity -- looks for and uses switch[state=true] to infer that a human is present,
            so will work with most binary switches without further modification. The state attribute should not be
            added manually to the configuration, as it will just be overwritten by the hardware at runtime.

Motion Detectors are configured as:
    <motion kind="<brand>" name="<identifier>" delay="<time>"></motion>

  Required attributes:
    ibid <switch>

  Optional attributes:
    ibid <switch>

  Runtime attributes:
    state - as for <switch>. Typically, there is an additional daemon -- oh_filter_motion -- that will take the raw,
            state and apply a hysteresis to the input to provide a more reliable "was there motion?" input via:

    filtered-state - a post-processed state that tries to take into account the inherently unstable nature of
                     motion detectors.
"""
import argparse
import asyncio
import functools
import logging
import shared.aiohome as aiohome
import shared.util as util
from pathlib import PurePath

log = logging.getLogger("oh_infer_activity")


class SwitchState:
    def __init__(self, path: str, node: aiohome.NodeData):
        self.path = path
        self.cached_node = node

        # List of contexts which want updates when our cached content changes.
        self.contexts_ = []

    @property
    def activity(self):
        if self.cached_node.attrs.get('state', 'false') == 'true':
            return self.cached_node.attrs.get('activity', 'yes')
        return 'unknown'

    def add_context(self, context):
        assert context not in self.contexts_
        self.contexts_.append(context)

    @asyncio.coroutine
    def on_change(self, home: aiohome.Home, path: str, node: aiohome.NodeData):
        assert path == self.path
        self.cached_node = node
        log.info("switch {} changed state to {}; applying to {}".format(node.name,
                                                                        node.attrs.get('state', 'unset'),
                                                                        [ctx.name for ctx in self.contexts_]))
        for context in self.contexts_:
            yield from context.on_state_changed(home)


class ActivityContext:
    def __init__(self, path: str, name: str):
        self.path = path
        self.name = name

        # The set of switches which control this context.
        self.switches_ = []

    def get_tightest_activity(self):
        seen_activity = False

        for switch in self.switches_:
            activity = switch.activity

            # No new information from this switch.
            if activity == 'unknown':
                continue

            # Activity is generic.
            if activity == 'yes':
                seen_activity = True
                continue

            # Activity is specific. We have no way to choose, so first one wins for now.
            return activity

        if seen_activity:
            return 'yes'
        return 'unknown'

    def add_switch(self, switch: SwitchState):
        assert switch not in self.switches_
        self.switches_.append(switch)

    @asyncio.coroutine
    def on_state_changed(self, home: aiohome.Home):
        activity = self.get_tightest_activity()
        log.info("{} changed activity to {}".format(self.path, activity))
        yield from home(home.path_to_query(self.path)).attr('activity', activity).run()


@asyncio.coroutine
def find_valid_contexts(home: aiohome.Home) -> {str: ActivityContext}:
    nodes = yield from home('home, room').run()
    return {path: ActivityContext(path, node.name) for path, node in nodes.items()}


@asyncio.coroutine
def get_context_paths_for_switch(home: aiohome.Home, contexts: {str, ActivityContext}, path: str,
                                 node: aiohome.NodeData) -> [str]:
    # If no context is specified, return the tightest bound context.
    if 'context' not in node.attrs:
        while path not in contexts:
            path = str(PurePath(path).parent)
            assert str(path) != '/'
        assert path in contexts
        return [contexts[path]]

    # Otherwise query with the contexts attr and return all matching contexts.
    query = node.attrs['context']
    nodes = yield from home(query).run()
    out = []
    for path, node in nodes.items():
        if path not in contexts:
            log.warning("The context configured on {}, '{}', refers to at least one non-valid context.".format(
                node.name, query))
            continue
        out.append(contexts[path])
    return out


@asyncio.coroutine
def main():
    parser = argparse.ArgumentParser(description='Interpret switch and motion states to infer human activity.')
    util.add_common_args(parser)
    args = parser.parse_args()

    util.enable_logging(args.log_target, args.log_level)
    home = yield from aiohome.connect(('localhost', 8080))

    # List all contexts that can have an 'activity' attribute.
    contexts = yield from find_valid_contexts(home)
    log.info("Found {} contexts".format(len(contexts)))

    # Iterate all switches and associate them with a context.
    switches = yield from home('switch, motion').run()
    log.info("Found {} switches".format(len(switches)))
    for path, node in switches.items():
        # Create a cache of each switch state, so that one switch's changes don't result in queries for other switches.
        switch = SwitchState(path, node)

        # Iterate all contexts, adding the switch as an input and the context as a target.
        switch_contexts = yield from get_context_paths_for_switch(home, contexts, path, node)
        for context in switch_contexts:
            log.info("binding switch {} to context {}".format(node.name, context.path))
            context.add_switch(switch)
            switch.add_context(context)

        # Subscribe to events on the switch.
        yield from home.subscribe(switch.path, functools.partial(switch.on_change, home))


if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(main())
    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass
