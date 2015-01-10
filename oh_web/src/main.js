var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');

var treeView = require('./tree');


var styles = jss.createStyleSheet({
    'body': {
        'color': '#AAAAAA',
        'background-color': '#000000',
    },
    '.tree-item-style': {
    },
    '.undefined-style': {
        'list-style': 'disc',
    },
    '.home-style': {
        'list-style-image': 'url(/resources/home32.png)',
    },
    '.room-style': {
        'list-style-image': 'url(/resources/room32.png)',
    },
    '.hue-style': {
        'list-style-image': 'url(/resources/hue32.png)',
    },
    '.huebridge-style': {
        'list-style': 'disc',
    },
    '.wemomotion-style': {
        'list-style-image': 'url(/resources/wemomotion32.png)',
    },
    '.wemoswitch-style': {
        'list-style': 'disc',
    },
    '.scene-style': {
        'list-style': 'disc',
    },
});
styles.attach();

function main() {
    $('body').empty().append('<div id="root">/<div></div></div>');
    treeView.attach($("#root > div"));
}
$(main);
