// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');

var home = require('./home');
var alarmView = require('./alarm');
var birdseyeView = require('./birdseye');
var switchView = require('./switch');
var treeView = require('./tree');

function main() {
    var styles = jss.createStyleSheet({
        'body': {
            'color': '#000',
            'background-color': '#FFF',
        },
        '#top-panel': {
            'border': '1px solid #0061cf',
            'border-radius': '5px',
            'padding': '5px',
        },
        '#tree': {
            'margin': '5px',
            'padding-top': '20px',
        }
    });
    styles.attach();

    $('body')
        .empty()
        .append(`<div id="top-panel">
                   <div id="switch"></div>
                   <div id="alarm"></div>
                 </div>
                 <div id="birdseye"></div>
                 <div id="tree">Raw Tree View:</div>`);

    home.connect(HOME_ADDRESS)
        .then(conn => {
            switchView.attach(conn, $("#switch"));
            //alarmView.attach(conn, $("#alarm"));
            birdseyeView.attach(conn, $("#birdseye"));
            treeView.attach(conn, $("#tree"));
        });
}
$(main);
