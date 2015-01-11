var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');

var switchView = require('./switch');
var treeView = require('./tree');

function main() {
    $('body')
        .empty()
        .append('<div id="switch"></div>'+
                '<div id="alarm"></div>'+
                '<div id="birdseye"></div>'+
                '<div id="tree">/<div></div></div>');

    switchView.attach($("#switch"));
    treeView.attach($("#tree > div"));
}
$(main);
