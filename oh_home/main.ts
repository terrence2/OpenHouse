/*
 * This Source Code Form is subject to the terms of the GNU General Public
 * License, version 3. If a copy of the GPL was not distributed with this file,
 * You can obtain one at https://www.gnu.org/licenses/gpl.txt.
 */
/// <reference path="interfaces/node.d.ts" />
/// <reference path="interfaces/bunyan.d.ts" />
/// <reference path="interfaces/jsdom.d.ts" />
/// <reference path="interfaces/primus.d.ts" />
/// <reference path="interfaces/yargs.d.ts" />
import fs = require('fs');

import bunyan = require('bunyan');
import jsdom = require('jsdom');
import primus = require('primus');
import yargs = require('yargs');

// Parse command line.
var args = yargs.usage("OpenHouse HOMe server.", {
    'help': {
        description: "Show this help message",
        boolean: true,
        alias: 'h'
    },
    'log-level': {
        description: "The logging level for stdout messages",
        type: 'string',
        default: 'info',
        alias: 'l'
    },
    'log-target': {
        description: "The debug logging target file",
        type: 'string',
        default: 'events.log',
        alias: 'L'
    },
    'address': {
        description: "The address to listen on",
        type: 'string',
        default: '0.0.0.0',
        alias: 'a'
    },
    'port': {
        description: "The port to listen on",
        type: 'string',
        default: 8887,
        alias: 'p'
    }
}).argv;
if (args.help) {
    yargs.showHelp();
    process.exit(0);
}


var log = bunyan.createLogger({
  name: 'HOMe',
  streams: [
    { level: args.logLevel.toLowerCase(), stream: process.stdout },
    { level: 'debug', path: args.logTarget }
  ]
});


// Protocol constants.
var query_major_version: number = 3;
var query_minor_version: number = 0;
// WebSocket constants.
var websocket_ipv4: string = args.address;
var websocket_port: number = args.port;
var websocket_address: string = "ws://" + websocket_ipv4 + ":" + websocket_port + "/primus";
var websocket_client_code: string = "http://" + websocket_ipv4 + ":" + websocket_port + "/primus/primus.js";
// Configuration constants.
var home_html: string = "home.xhtml";
var autosave_interval: number = 5 * 60 * 1000;


function print_usage(message) {
    console.log("Error: " + message);
    console.log("");
    console.log("Usage: node oh_home.js <home.html>");
}
if (process.argv.length < 3) {
    print_usage("No home.html provided.");
    process.exit(1);
}
home_html = process.argv[2];


function pathof($, node): string {
    var path = [];
    var current = $(node);
    while (current.attr("name")) {
        path.unshift(current.attr("name"));
        current = $(current.parent()[0]);
    }
    if (!path.length)
        return "";
    return "/" + path.join("/");
}


interface Subscriptions {
    [index: string]: Array<any>;
}
class Context {
    $: any; // I'm not even going to try to type jquery.
    window: any; // or jsdom.
    ws_subscriptions: Subscriptions;

    constructor(window) {
        this.window = window;
        this.$ = window.$;
        this.ws_subscriptions = {};
    }
}


interface Message {
    type: string;
}
interface Response {
    error?: string;
    exception?: any;
}
function handle_message(ctx: Context, data): Response {
    var type: string = data.type;
    try {
        if (type == "ping")
            return handle_ping(data);
        else if (type == "html")
            return handle_html(ctx);
        else if (type == "query")
            return handle_query(ctx, data);
    } catch(e) {
        log.error({exception: e}, "Failed to handle message");
        return {error: "Unexpected exception", exception: e};
    }

    log.warn({message_type: type}, "unrecognized message type");
    return {error: "Unrecognized message type: " + type}
}


interface PingMessage extends Message {
    ping: string;
}
interface Version {
    major: number;
    minor: number;
}
interface PingResponse extends Response {
    pong: string;
    version: Version;
}
function handle_ping(data: PingMessage): PingResponse {
    log.info("handling ping");
    var version: Version = { major: query_major_version, minor: query_minor_version };
    var result: PingResponse = {
        pong: data.ping,
        version: version,
    };
    return result;
}


interface HtmlResponse extends Response {
    html: string;
}
function handle_html(ctx: Context): HtmlResponse {
    log.info("returning html serialization");
    return {html: jsdom.serializeDocument(ctx.window.document)};
}


interface Transform {
    method: string;
    args: string[];
}
interface Query {
    query: string;
    transforms: Transform[];
}
interface QueryMessage extends Message {
    query_group: Query[];
}
interface Attributes {
    [index: string]: string;
}
function get_attrs($, node): Attributes {
    var attrs: Attributes = {};
    $(node.attributes).each(function() { attrs[this.nodeName] = this.nodeValue; });
    return attrs;
}
interface QueryResult {
    text: string;
    tagName: string;
    attrs: Attributes;
    styles: Attributes;
}
function get_matching_css(ctx, elem) {
    var sheets:CSSStyleSheet = ctx.window.document.styleSheets;
    var matching_css = [];
    for (var i in sheets) {
        var rules = sheets[i].cssRules;
        for (var r in rules) {
            if (elem.matchesSelector(rules[r].selectorText)) {
                matching_css.push(rules[r].style);
            }
        }
    }
    return matching_css;
}
function css_style_to_map(css, styles: Attributes) {
    for (var i = 0; i < css.length; ++i) {
        var key = css[i];
        styles[key] = css.getPropertyValue(key);
    }
}
function get_styles(ctx: Context, elem): Attributes {
    var styles: Attributes = {};
    var matching = get_matching_css(ctx, elem);
    for (var i in matching) {
        css_style_to_map(matching[i], styles);
    }
    return styles;
}
function to_result(ctx: Context, node): QueryResult {
    // Text, with text of children stripped out.
    var text = ctx.$(node).clone().children().remove().end().text().trim();
    var tagName = ctx.$(node).prop('tagName');
    var attrs = get_attrs(ctx.$, node);
    var styles = get_styles(ctx, ctx.$(node)[0]);
    return { attrs: attrs, styles: styles, text: text, tagName: tagName };
}
interface QueryResponse {
    [index: string]: QueryResult;
}
function handle_query(ctx: Context, data: QueryMessage): QueryResponse {
    log.debug("handling query group");

   // Keep a list of all touched and changed nodes for output and subscription updates.
    var touched: QueryResponse = {};
    var changed: QueryResponse = {};

    // Handle equy query in order.
    for (var i in data.query_group)
        handle_one_query(ctx, data.query_group[i], touched, changed);

    // Publish touched nodes for subscribers to snoop on.
    for (var path in changed) {
        if (path === '')
            continue;

        if (ctx.ws_subscriptions[path] !== undefined) {
            log.debug("dispatching %s change to %d subscriptions", path,
                      ctx.ws_subscriptions[path].length);
            for (var i in ctx.ws_subscriptions[path]) {
                var spark = ctx.ws_subscriptions[path][i];
                log.debug("sending change to %s:%s", spark.address.ip, spark.address.port);
                spark.write({path: path, message: changed[path]});
            }
        }
    }

    delete touched[''];
    return touched;
}
function handle_one_query(ctx: Context, query: Query, touched: QueryResponse, changed: QueryResponse) {
    log.debug({query:query}, "handling query");

    // Perform the base query.
    var nodes = ctx.$(query.query);
    nodes.each(function(i, node) { touched[pathof(ctx.$, node)] = to_result(ctx, node); });

    // Apply each transform to the initial query.
    for (var i in query.transforms) {
        var method_name = query.transforms[i].method;
        var args = query.transforms[i].args;

        nodes = nodes[method_name].apply(nodes, args);
        nodes.each(function(i, node) {
            var map = to_result(ctx, node);
            touched[pathof(ctx.$, node)] = map;
            changed[pathof(ctx.$, node)] = map;
        });
    }
}


interface SubscribeMessage extends Message {
    target: string;
}
interface SubscribeResponse extends Response {
}
function handle_subscribe(ctx: Context, data: SubscribeMessage, spark): SubscribeResponse {
    if (ctx.ws_subscriptions[data.target] === undefined)
        ctx.ws_subscriptions[data.target] = [];
    ctx.ws_subscriptions[data.target].push(spark);
    log.debug("added subscription to %s -- %s total", data.target,
              ctx.ws_subscriptions[data.target].length);
    return {};
}


function loaded_jsdom(errors, window)
{
    if (errors) {
        log.fatal("failed to load %s: %s", home_html, errors.toString());
        process.exit(1);
    }
    log.info("Loaded home: " + home_html);

    // Setup broadcast sockets and listen for connections via WebSockets.
    var ctx = new Context(window);

    // Listen for websocket connections.
    var primus_server = primus.createServer({
        hostname: websocket_ipv4,
        port: websocket_port,
        protocol: "JSON",
        timeout: false
    });
    log.info("Listening on " + websocket_ipv4 + ":" + websocket_port);
    primus_server.on('connection', function (spark) {
        log.info({address: spark.address}, 'new primus connection');

        spark.on('data', function (data) {
            var token = data.token;
            var message = data.message;
            log.debug({len: message.length}, 'handling websocket message');
            var output;
            if (message.type == 'subscribe')
                output = handle_subscribe(ctx, message, spark);
            else
                output = handle_message(ctx, message);
            spark.write({token: token, message: output});
        });
    });
    primus_server.on('disconnection', function(spark) {
        for (var key in ctx.ws_subscriptions) {
            ctx.ws_subscriptions[key] = ctx.ws_subscriptions[key].filter(
                                            function(val, i, arr) { return val !== spark; });
            if (ctx.ws_subscriptions[key].length === 0) {
                delete ctx.ws_subscriptions[key];
            }
        }
    });
}


fs.readFile(home_html, function(error, data) {
    if (error) {
        log.fatal("Failed to to load home '" + home_html + "': " + error);
        process.exit(1);
    }

    jsdom.env(
        data.toString(),
        ["../node_modules/jquery/dist/jquery.min.js",
         "../node_modules/jquery-color/jquery.color.js",
        ],
        loaded_jsdom
    );
});
