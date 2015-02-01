var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');

var home = require('./home');
var alarmView = require('./alarm');
var switchView = require('./switch');
var treeView = require('./tree');

var gHandlers = [];
function broadcast_handler(path, msg)
{
    for (var i in gHandlers)
        gHandlers[i](path, msg);
}

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

    home.connect(HOME_ADDRESS, broadcast_handler)
        .then(conn => {
            gHandlers.push(switchView.attach(conn, $("#switch")));
            gHandlers.push(alarmView.attach(conn, $("#alarm")));
            gHandlers.push(treeView.attach(conn, $("#tree")));
        });
}
$(main);
