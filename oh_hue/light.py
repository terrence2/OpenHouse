# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
import asyncio
import logging
import json
import shared.aiohome as aiohome

from shared.color import parse_css_color, Color, RGB, BHS, Mired
from bridge import Bridge


class Light:
    def __init__(self, id_: str, path: str, name: str, bridge: Bridge, node: aiohome.NodeData):
        # Log to a logger with our exact name in it.
        self.log = logging.getLogger("oh_hue.light.{}".format(name))

        # The light's hue-side id.
        self.id_ = id_

        # The light's open-house side name and path.
        self.name_ = name
        self.path_ = path

        # The controlling bridge.
        self.bridge_ = bridge

        # The current open-house side light state.
        self.color, self.on, self.transitiontime = Mired(52), True, 10
        self.set_light_state_from_oh(node)

    @classmethod
    @asyncio.coroutine
    def create(cls, id_: str, path: str, node: aiohome.NodeData, bridge: Bridge, home: aiohome.Home):
        light = cls(id_, path, node.name, bridge, node)
        yield from home.subscribe(path, light.on_change)
        light.log.info("subscribed to light at {}".format(path))
        return light

    def set_light_state_from_oh(self, node: aiohome.NodeData) -> (Color, bool, int):
        try:
            style = node.attrs['style']
        except KeyError as ex:
            self.log.warn("No style set on light.")
            return

        # FIXME: use DOM to getComputedStyle so that we can use CSS, etc.
        css = {}
        parts = style.strip(';').split(';')
        for part in parts:
            key, _, value = part.strip().partition(':')
            css[key.strip()] = value.strip()

        try:
            color = parse_css_color(css.get('color', 'rgb(255,255,255)'))
            light_vis = css.get('visibility', 'visible')
            trans_time = int(float(css.get('animation-duration', 1)) * 10)
        except Exception as ex:
            self.log.error("Failed to parse light style")
            self.log.exception(ex)
            return

        # Color must be BHS or Mired.
        if isinstance(color, RGB):
            color = BHS.from_rgb(color)

        self.color = color
        self.on = light_vis == 'visible'
        self.transitiontime = trans_time
        return (self.color,
                self.on,
                self.transitiontime)

    @asyncio.coroutine
    def on_change(self, target: str, data: aiohome.NodeData):
        assert target == self.path_
        self.set_light_state_from_oh(data)
        self.log.info("applying light state: {}".format(str(self)))

        props = {'on': self.on}
        if self.on:
            props.update({'transitiontime': self.transitiontime})
            if isinstance(self.color, BHS):
                props.update({
                    'bri': self.color.b,
                    'hue': self.color.h,
                    'sat': self.color.s,
                })
            else:
                assert isinstance(self.color, Mired)
                props.update({'ct': self.color.ct})

        json_data = json.dumps(props).encode('UTF-8')
        yield from self.bridge_.set_light_state(self.id_, json_data)

    def __str__(self):
        if self.on:
            return "ON | {} for {}s".format(self.color, self.transitiontime / 10)
        return 'OFF'

