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
    return 0;
}

var METERS_TO_PX = 100;

function get_display_size(size) {
    // -1 to account for 1px borders.
    return parse_size(size) * METERS_TO_PX - 1;
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

    conn.query('[activity]').run()
        .then(activities_msg => {
            var activities = ["yes", "unknown"];
            for (var path in activities_msg)
                activities.push(activities_msg[path].attrs.activity);
            conn.query('home > room').run()
                .then(msg => display_rooms(msg, activities, e, conn));
        });
}

function display_rooms(rooms_msg, activities, elem, conn)
{
    for (var path in rooms_msg)
        display_room(rooms_msg[path], activities, elem, conn);
}

function display_room(data, activities, elem, conn)
{
    var room_name = data.attrs.name;

    // Create the basic room shape.
    var e = $(`<div class="birdseye-room">${room_name}<br/></div>`, {})
        .width(get_display_size(data.attrs.w))
        .height(get_display_size(data.attrs.l))
        .css('position', 'absolute')
        .css('left', get_display_offset(data.attrs.x, $(elem).offset().left))
        .css('top', get_display_offset(data.attrs.y, $(elem).offset().top))
        .appendTo(elem);

    var cnt = 0;

    // Unless it has noborder, draw an outline around it.
    if (data.attrs.noborder === undefined)
        e.css('border', '1px solid black');

    // Find and draw any closets.
    conn.query(`room[name=${room_name}] > closet`).run()
        .then((msg) => {
            for (var path in msg)
                display_closet(msg[path], e);
        });

    // Create and populate the scene selection dropdown in each room.
    var sel = $(`<select id="birdseye-room-${room_name}-select"></select>`)
        .appendTo(e);
    R.map((v) => $(sel).append(`<option value="${v}">${v}</option>`), activities);
    // Get the current value of the room's activity.
    conn.query(`room[name=${room_name}]`).run()
        .then(msg => {
            var path = R.last(R.keys(msg));
            var data = R.last(R.values(msg));
            $(sel).val(data.attrs.activity || 'unknown');

            // Listen for future changes.
            conn.subscribe(path, (_, msg) => {
                var activity = msg.attrs.activity || 'unknown';
                var color = activity == 'unknown' ? '' : '#d7ffea';
                $(e).css('background-color', color);
                $(sel).val(activity);
            });
        });
    sel.on('change', (e) => {
        var switchValue = e.target.options[e.target.selectedIndex].value;
        console.log(`Changing room ${room_name} to activity ${switchValue}`);
        conn.query(`room[name=${room_name}]`).attr('activity', switchValue).run();
    });
}

function display_closet(data, room_elem)
{
    var w = get_display_size(data.attrs.w);
    var l = get_display_size(data.attrs.l);
    var x = get_display_offset(data.attrs.x, 0);
    var y = get_display_offset(data.attrs.y, 0);

    if (x <= 0 || x >= $(room_elem).offset().left) x -= 1;
    if (y <= 0 || y >= $(room_elem).offset().top) y -= 1;

    var e = $('<div class="birdseye-closet"/>', {})
        .width(w)
        .height(l)
        .css('position', 'absolute')
        .css('left', x)
        .css('top', y)
        .appendTo(room_elem);
    if (data.attrs.noborder === undefined)
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
        }
    });
    styles.attach();

    conn.query('home').run()
        .then(msg => create_home_area(R.values(msg)[0], elem, conn));
}

module.exports = {
    attach: attach
};

