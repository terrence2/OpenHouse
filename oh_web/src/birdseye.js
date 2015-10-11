// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var $ = require('jquery');
var R = require('ramda');
var jss = require('jss');
var home = require('./home');
var util = require('./util');

function parse_size(size) {
    var FEET_TO_METERS = 0.3048;
    var groups = size.match(/(-?\d+)ft/);
    if (groups)
        return Number(groups[1]) * FEET_TO_METERS;
    var groups = size.match(/(\d+)ft(\d+)in/);
    if (groups)
        return (Number(groups[1]) + (Number(groups[2]) / 12)) * FEET_TO_METERS;
    var groups = size.match(/(-?\d+)in/);
    if (groups)
        return (Number(groups[1]) / 12) * FEET_TO_METERS;
    return 0;
}

var METERS_TO_PX = 100;

function get_display_size(size) {
    // -1 to account for 1px borders.
    return parseInt(Math.round(parse_size(size) * METERS_TO_PX - 1));
}

function get_display_offset(size, offset) {
    var px = parse_size(size) * METERS_TO_PX;
    return px + offset;
}

function create_home_area(data, elem, conn) {
    var e = $("<div/>", {id: 'birdseye-home-' + data.attrs.name})
        .css('margin', 20)
        .width(get_display_size(data.attrs.w))
        .height(get_display_size(data.attrs.l))
        .appendTo(elem);

    conn.query('home > design').run()
        .then(R.values)
        .then(R.map(v => v.attrs.name))
        .then((global_designs) => {
            conn.query('home > room').run()
                .then(R.mapObj(R.curry(display_room)(global_designs, elem, conn)));
        });
}

function display_room(global_designs, elem, conn, node)
{
    var room_name = node.attrs.name;
    var home_offset_left = $(elem).offset().left;
    var home_offset_top = $(elem).offset().top;

    // Create the basic room shape.
    var room_elem = $(`<div class="birdseye-room">${room_name}<br/></div>`, {})
        .width(get_display_size(node.attrs.w))
        .height(get_display_size(node.attrs.l))
        .css('position', 'absolute')
        .css('left', get_display_offset(node.attrs.x, home_offset_left))
        .css('top', get_display_offset(node.attrs.y, home_offset_top))
        .appendTo(elem);

    var cnt = 0;

    // Unless it has noborder, draw an outline around it.
    if (node.attrs.noborder === undefined)
        room_elem.css('border', '1px solid black');

    // Find and draw any closets.
    conn.query(`room[name=${room_name}] > closet`).run()
        .then(R.mapObj(R.curry(display_closet)(room_elem)));

    // Create and populate the design selection dropdown in each room.
    var sel = $(`<select id="birdseye-room-${room_name}-select"></select>`)
        .appendTo(room_elem);
    $(sel).append('<option value="__unset__"></option>');
    R.map((v) => $(sel).append(`<option value="${v}">${v}</option>`), global_designs);

    // Get the current value of the room's activity.
    conn.query(`room[name=${room_name}]`).run()
        .then(msg => {
            var path = R.last(R.keys(msg));
            var data = R.last(R.values(msg));

            // Update the dropdown to the current value.
            $(sel).val(data.attrs.design || '__unset__');

            // Listen for future changes.
            conn.subscribe(path, (_, msg) => {
                var activity = msg.attrs.activity || 'unknown';
                console.log("updating room " + path + " to " + activity);
                var color = '';
                if (activity == 'yes') color = '#d7ffea';
                if (activity == 'no') color = '#ffead7';
                $(room_elem).css('background-color', color);
                $(sel).val(activity);
            });
        });
    sel.on('change', (e) => {
        var switchValue = e.target.options[e.target.selectedIndex].value;
        console.log(`Changing room ${room_name} to design ${switchValue}`);
        conn.query(`room[name=${room_name}]`).attr('design', switchValue).run();
    });

    // Overlay motion detectors and switches.
    conn.query(`room[name=${room_name}] motion, room[name=${room_name}] switch`).run()
        .then(R.mapObjIndexed((node, path, msg) => {
            var name = node.attrs.name;
            var tagname = node.tagName.toLowerCase();
            var motion_elem = $(`<div class="birdseye-${tagname}"><span>${name}</span></div>`)
                .css('position', 'absolute')
                .css('left', get_display_offset(node.attrs.x, -15))
                .css('top', get_display_offset(node.attrs.y, -15))
                .appendTo(room_elem);
            conn.subscribe(path, (msg_path, msg_node) => {
                if (msg_node.attrs['raw-state'] == 'true')
                    motion_elem.addClass('active');
                else
                    motion_elem.removeClass('active');
            });
        }));
}

function display_closet(room_elem, node)
{
    var w = get_display_size(node.attrs.w);
    var l = get_display_size(node.attrs.l);
    var x = get_display_offset(node.attrs.x, 0);
    var y = get_display_offset(node.attrs.y, 0);

    if (x <= 0 || x >= $(room_elem).offset().left) x -= 1;
    if (y <= 0 || y >= $(room_elem).offset().top) y -= 1;

    var e = $('<div class="birdseye-closet"/>', {})
        .width(w)
        .height(l)
        .css('position', 'absolute')
        .css('left', x)
        .css('top', y)
        .appendTo(room_elem);
    if (node.attrs.noborder === undefined)
        e.css('border', '1px solid black');
}

function attach(conn, elem)
{
    var styles = jss.createStyleSheet({
        '.birdseye-room > select': {
            'margin-left': '20px'
        },
        '.birdseye-room:hover': {
            'background-color': '#EEEEFF'
        },
        '.birdseye-motion': {
            'background-image': 'url(/resources/wemomotion32.png)',
            'padding': '0px',
            'width': '32px',
            'height': '32px',
        },
        '.birdseye-motion.active': {
            'background-image': 'url(/resources/wemomotion32_active.png)',
        },
        '.birdseye-motion > span': {
            'display': 'none'
        },
        '.birdseye-motion:hover > span': {
            'display': 'inline'
        },
        '.birdseye-switch > span': {
            'display': 'none'
        },
        '.birdseye-switch:hover > span': {
            'display': 'inline'
        },
        '.birdseye-switch': {
            'background-image': 'url(/resources/wemoswitch32.png)',
            'padding': '0px',
            'width': '32px',
            'height': '32px',
        },
        '.birdseye-switch.active': {
            'background-image': 'url(/resources/wemoswitch32_active.png)',
        },
    });
    styles.attach();

    conn.query('home').run()
        .then(msg => create_home_area(R.values(msg)[0], elem, conn));
}

module.exports = {
    attach: attach
};

